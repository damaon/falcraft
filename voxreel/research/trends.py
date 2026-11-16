"""Trend scan for kid-friendly Minecraft / voxel YouTube Shorts.

1. discovery: Shorts from hashtag pages (youtube.com/hashtag/<tag>/shorts) per query group, plus
   (with YOUTUBE_API_KEY) the most viewed fresh uploads from API search (`[search]` in config)
2. details (upload date, likes, tags...): YouTube Data API when YOUTUBE_API_KEY is set (fast,
   official), otherwise yt-dlp one video at a time, slowly (YouTube blocks bursts as a bot)
3. kid-safe filter: drops brainrot / gross-out / scary videos (config `filter.blocklist`)
4. ranking: uploads inside the period window by views per day
5. channel outliers: newest Shorts of the top channels vs the channel's median views
6. sounds: Shazam on the audio of the top videos (cached per video)

Output: out/<date>/report.md + trends.json. Run: `mise run trends` (from voxreel/).
"""
from __future__ import annotations

import argparse
import asyncio
import collections
import datetime as dt
import json
import os
import re
import statistics
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
CACHE = HERE / ".cache"
YTDLP = [sys.executable, "-m", "yt_dlp", "--js-runtimes", "node", "--no-warnings", "--ignore-errors"]
FIELDS = (
    "id,title,channel,channel_id,channel_url,upload_date,timestamp,view_count,like_count,comment_count,"
    "duration,tags,categories,age_limit,description"
)
PERIOD_DAYS = {"day": 1, "week": 7, "month": 30}
STOPWORDS = set(
    "the a an and or of in on to for with is it this that i my you your me we us at by from be are was "
    "how what why when best new vs but not no so just make made can do get all one minecraft shorts "
    "short youtube youtubeshorts viral fyp video".split()
)


class BotCheck(Exception):
    """YouTube answered "Sign in to confirm you're not a bot": stop hitting watch pages."""


def ytdlp(args: list[str], timeout: int = 180) -> list[dict]:
    try:
        r = subprocess.run(YTDLP + args, capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        return []
    if "not a bot" in r.stderr:
        raise BotCheck()
    rows = []
    for line in r.stdout.splitlines():
        if line.startswith("{"):
            try:
                rows.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return rows


def flat_list(url: str, limit: int) -> list[dict]:
    """Entries of a hashtag page or channel tab without opening each video."""
    try:
        return ytdlp(["--flat-playlist", "--playlist-end", str(limit), "--print", "%(.{id,title,channel,channel_url,view_count})j", url])
    except BotCheck:
        return []


# ---------------------------------------------------------------------------- details
def iso_duration(s: str) -> int | None:
    m = re.fullmatch(r"P(?:(\d+)D)?T?(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?", s or "")
    if not m:
        return None
    d, h, mi, se = (int(x or 0) for x in m.groups())
    return ((d * 24 + h) * 60 + mi) * 60 + se


def details_api(ids: list[str], key: str) -> list[dict]:
    out = []
    for i in range(0, len(ids), 50):
        q = urllib.parse.urlencode({"part": "snippet,statistics,contentDetails,status", "id": ",".join(ids[i : i + 50]), "key": key})
        with urllib.request.urlopen(f"https://www.googleapis.com/youtube/v3/videos?{q}", timeout=30) as r:
            items = json.load(r).get("items", [])
        for it in items:
            sn, st, cd = it["snippet"], it.get("statistics", {}), it.get("contentDetails", {})
            published = dt.datetime.fromisoformat(sn["publishedAt"].replace("Z", "+00:00"))
            out.append({
                "id": it["id"], "title": sn.get("title"), "description": sn.get("description"), "tags": sn.get("tags") or [],
                "channel": sn.get("channelTitle"), "channel_id": sn.get("channelId"),
                "channel_url": f"https://www.youtube.com/channel/{sn.get('channelId')}",
                "timestamp": published.timestamp(), "duration": iso_duration(cd.get("duration")),
                "view_count": int(st.get("viewCount", 0)), "like_count": int(st["likeCount"]) if "likeCount" in st else None,
                "comment_count": int(st["commentCount"]) if "commentCount" in st else None,
                "age_limit": 18 if cd.get("contentRating", {}).get("ytRating") == "ytAgeRestricted" else 0,
                "made_for_kids": it.get("status", {}).get("madeForKids"),
            })
    return out


def search_api(query: str, key: str, after: dt.datetime, n: int = 50) -> list[str]:
    """Most viewed short videos uploaded after `after` (100 quota units per call)."""
    q = urllib.parse.urlencode({
        "part": "id", "q": query, "type": "video", "videoDuration": "short", "order": "viewCount",
        "publishedAfter": after.strftime("%Y-%m-%dT%H:%M:%SZ"), "maxResults": n, "safeSearch": "strict", "key": key,
    })
    with urllib.request.urlopen(f"https://www.googleapis.com/youtube/v3/search?{q}", timeout=30) as r:
        return [it["id"]["videoId"] for it in json.load(r).get("items", []) if it.get("id", {}).get("videoId")]


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


def is_short(video_id: str) -> bool:
    """youtube.com/shorts/<id> answers 200 for a Short and redirects to /watch for a normal video."""
    req = urllib.request.Request(f"https://www.youtube.com/shorts/{video_id}", method="HEAD")
    try:
        with urllib.request.build_opener(_NoRedirect).open(req, timeout=20) as r:
            return r.status == 200
    except urllib.error.HTTPError:
        return False
    except Exception:
        return True  # network trouble: don't drop the video over it


def details_ytdlp(ids: list[str], pause: float, notes: list[str]) -> list[dict]:
    out = []
    for n, vid in enumerate(ids, 1):
        try:
            rows = ytdlp(["--skip-download", "--print", f"%(.{{{FIELDS}}})j", f"https://www.youtube.com/watch?v={vid}"], timeout=90)
        except BotCheck:
            notes.append(f"YouTube bot check after {n - 1} of {len(ids)} videos: details incomplete. Set YOUTUBE_API_KEY or try again later.")
            break
        out += rows
        print(f"  details {n}/{len(ids)}", end="\r", file=sys.stderr)
        time.sleep(pause)
    print(file=sys.stderr)
    return out


def oembed(video_id: str) -> dict | None:
    """Title + channel from the oEmbed endpoint (works even when watch pages ask for sign-in)."""
    url = "https://www.youtube.com/oembed?" + urllib.parse.urlencode({"url": f"https://www.youtube.com/shorts/{video_id}", "format": "json"})
    try:
        with urllib.request.urlopen(url, timeout=20) as r:
            d = json.load(r)
    except Exception:
        return None
    return {"id": video_id, "title": d.get("title"), "channel": d.get("author_name"), "channel_url": d.get("author_url")}


# ---------------------------------------------------------------------------- music
def identify_music(videos: list[dict], pause: float, notes: list[str]) -> dict[str, dict]:
    """Shazam on each video's audio; results cached per video id."""
    cache_file = CACHE / "music.json"
    cache = json.loads(cache_file.read_text()) if cache_file.exists() else {}
    audio_dir = CACHE / "audio"
    audio_dir.mkdir(parents=True, exist_ok=True)
    files = {}
    for v in [v for v in videos if v["id"] not in cache]:
        target = audio_dir / f"{v['id']}.mp3"
        if not target.exists():
            try:
                ytdlp(["-q", "-f", "bestaudio/best", "-x", "--audio-format", "mp3", "-o", str(audio_dir / "%(id)s.%(ext)s"),
                       f"https://www.youtube.com/shorts/{v['id']}"])
            except BotCheck:
                notes.append("YouTube bot check while downloading audio: some sounds not identified.")
                break
            time.sleep(pause)
        if target.exists():
            files[v["id"]] = target

    async def recognize_all():
        from shazamio import Shazam

        shazam = Shazam()
        for vid, path in files.items():
            try:
                r = await shazam.recognize(str(path))
                t = r.get("track") or {}
                cache[vid] = {"title": t.get("title"), "artist": t.get("subtitle"), "url": t.get("url")}
                path.unlink(missing_ok=True)
            except Exception as e:  # network hiccup: keep the audio and retry next run
                print(f"  shazam failed for {vid}: {e}", file=sys.stderr)

    if files:
        asyncio.run(recognize_all())
        CACHE.mkdir(exist_ok=True)
        cache_file.write_text(json.dumps(cache, indent=1, ensure_ascii=False))
    return cache


# ---------------------------------------------------------------------------- helpers
def blocked(video: dict, patterns: list[tuple[str, re.Pattern]]) -> str | None:
    text = " ".join([video.get("title") or "", " ".join(video.get("tags") or []), video.get("description") or ""])
    for word, p in patterns:
        if p.search(text):
            return word
    if (video.get("age_limit") or 0) > 0:
        return "age limit"
    return None


def hashtags(video: dict) -> list[str]:
    return [h.lower() for h in re.findall(r"#(\w+)", f"{video.get('title') or ''} {video.get('description') or ''}")]


def title_words(title: str) -> list[str]:
    return [w for w in re.findall(r"[a-z][a-z']+", (title or "").lower()) if w not in STOPWORDS and len(w) > 2]


def age_days(video: dict, now: dt.datetime) -> float | None:
    if video.get("timestamp"):
        return max((now.timestamp() - video["timestamp"]) / 86400, 0.25)
    if video.get("upload_date"):
        d = dt.datetime.strptime(video["upload_date"], "%Y%m%d").replace(tzinfo=dt.timezone.utc)
        return max((now - d).total_seconds() / 86400, 0.5)
    return None


def fmt_n(n) -> str:
    if n is None:
        return "?"
    for unit, d in (("B", 1e9), ("M", 1e6), ("k", 1e3)):
        if n >= d:
            return f"{n / d:.1f}{unit}"
    return f"{n:.0f}"


def esc(s) -> str:
    return str(s or "").replace("|", "/").replace("\n", " ").strip()


def music_label(m: dict | None) -> str:
    if not m:
        return "?"
    return f"{m['title']} - {m['artist']}" if m.get("title") else "(original / not recognised)"


# ---------------------------------------------------------------------------- report
def video_table(L: list[str], videos: list[dict], music: dict) -> None:
    L.append("| # | views/day | views | age | len | likes% | title | channel | group | sound |")
    L.append("|---|---|---|---|---|---|---|---|---|---|")
    for i, v in enumerate(videos, 1):
        likes = f"{100 * v['like_count'] / v['view_count']:.1f}" if v.get("like_count") and v.get("view_count") else "?"
        L.append(
            f"| {i} | {fmt_n(v['vpd'])} | {fmt_n(v['view_count'])} | {v['age']:.1f}d | {v.get('duration') or '?'}s | {likes} "
            f"| [{esc(v['title'])[:70]}](https://www.youtube.com/shorts/{v['id']}) | {esc(v.get('channel'))} "
            f"| {', '.join(v['groups'])} | {esc(music_label(music.get(v['id'])))} |"
        )


def write_report(path: Path, ctx: dict) -> None:
    cfg, recent, older, music, undated = ctx["cfg"], ctx["recent"], ctx["older"], ctx["music"], ctx["undated"]
    allv = recent + older
    L = [f"# Trends {ctx['date']} (window: {cfg['period']})\n"]
    L.append(
        f"{ctx['candidates']} Shorts found on hashtag pages, details for {ctx['detailed']} ({ctx['mode']}); "
        f"{len(ctx['excluded'])} filtered out as not kid-friendly; {len(recent)} uploaded in the window, {len(older)} older, "
        f"{len(undated)} without upload date (title only).\n"
    )
    for n in ctx["notes"]:
        L.append(f"> ⚠️ {n}\n")

    L.append(f"## Hot right now (uploaded in the last {cfg['period']}, by views per day)\n")
    video_table(L, recent[: cfg["top"]], music) if recent else L.append("Nothing in the window.")
    L.append("\n## Evergreen (older Shorts still on top of the hashtag pages)\n")
    video_table(L, older[:15], music) if older else L.append("None.")

    if undated:
        L.append("\n## Most viewed without details (no upload date, views rounded by YouTube)\n")
        L.append("| # | views | title | channel | group |")
        L.append("|---|---|---|---|---|")
        for i, v in enumerate(undated[: cfg["top"]], 1):
            L.append(f"| {i} | {fmt_n(v['view_count'])} | [{esc(v['title'])[:70]}](https://www.youtube.com/shorts/{v['id']}) | {esc(v['channel'])} | {', '.join(v['groups'])} |")

    L.append("\n## Formats (query groups)\n")
    L.append("| group | recent Shorts | median views/day | best views/day | all Shorts median views |")
    L.append("|---|---|---|---|---|")
    for g in cfg["groups"]:
        vs = [v["vpd"] for v in recent if g in v["groups"]]
        av = [v.get("view_count") or 0 for v in allv + undated if g in v["groups"]]
        L.append(f"| {g} | {len(vs)} | {fmt_n(statistics.median(vs)) if vs else '-'} | {fmt_n(max(vs)) if vs else '-'} | {fmt_n(statistics.median(av)) if av else '-'} |")

    sounds = collections.defaultdict(lambda: {"n": 0, "views": 0, "url": None})
    for v in allv:
        m = music.get(v["id"])
        if m and m.get("title"):
            s = sounds[music_label(m)]
            s["n"] += 1
            s["views"] += v.get("view_count") or 0
            s["url"] = m.get("url")
    checked = [v for v in allv if v["id"] in music]
    L.append(f"\n## Sounds ({len(checked)} top videos checked with Shazam)\n")
    if sounds:
        L.append("| sound | videos | total views |")
        L.append("|---|---|---|")
        for name, s in sorted(sounds.items(), key=lambda kv: (-kv[1]["n"], -kv[1]["views"])):
            label = f"[{esc(name)}]({s['url']})" if s["url"] else esc(name)
            L.append(f"| {label} | {s['n']} | {fmt_n(s['views'])} |")
    unknown = sum(1 for v in checked if not music[v["id"]].get("title"))
    L.append(f"\n{unknown} of {len(checked)} checked have original / unrecognised audio (voice-over, own music or sped-up edits).")

    top = (recent + older + undated)[:60]
    durations = [v["duration"] for v in top if v.get("duration")]
    tags = collections.Counter(h for v in top for h in set(hashtags(v)))
    words = collections.Counter(w for v in top for w in set(title_words(v["title"])))
    emoji = sum(1 for v in top if re.search(r"[\U0001F300-\U0001FAFF☀-➿]", v["title"] or ""))
    kids = [v.get("made_for_kids") for v in top if v.get("made_for_kids") is not None]
    L.append(f"\n## Packaging (top {len(top)})\n")
    if durations:
        L.append(f"- duration: median {statistics.median(durations):.0f}s, range {min(durations)}-{max(durations)}s")
    L.append(f"- title words: {', '.join(f'{w} ({n})' for w, n in words.most_common(25))}")
    L.append(f"- hashtags: {', '.join(f'#{h} ({n})' for h, n in tags.most_common(20))}")
    L.append(f"- titles with emoji: {emoji}/{len(top)}")
    if kids:
        L.append(f"- marked made for kids: {sum(kids)}/{len(kids)}")

    L.append("\n## Channel outliers (recent Shorts far above the channel's median)\n")
    if ctx["outliers"]:
        L.append("| x median | views | title | channel |")
        L.append("|---|---|---|---|")
        for o in ctx["outliers"][:20]:
            L.append(f"| {o['ratio']:.1f}x | {fmt_n(o['view_count'])} | [{esc(o.get('title'))[:70]}](https://www.youtube.com/shorts/{o['id']}) | {esc(o.get('channel'))} |")
    else:
        L.append("None.")

    reasons = collections.Counter(r for _, r in ctx["excluded"])
    L.append("\n## Filtered out (not kid-friendly)\n")
    L.append(", ".join(f"{r} ({n})" for r, n in reasons.most_common()) or "Nothing.")
    path.write_text("\n".join(L) + "\n")


# ---------------------------------------------------------------------------- main
def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--config", default=str(HERE / "config.toml"))
    ap.add_argument("--period", choices=list(PERIOD_DAYS), help="override config period")
    ap.add_argument("--details", type=int, help="override how many candidates get details")
    ap.add_argument("--no-music", action="store_true", help="skip Shazam")
    ap.add_argument("--cookies-from-browser", metavar="BROWSER",
                    help="let yt-dlp use your browser's YouTube login (avoids bot checks; uses your account)")
    ap.add_argument("--out", default=str(HERE / "out"))
    args = ap.parse_args()

    cfg = tomllib.loads(Path(args.config).read_text())
    cfg["period"] = args.period or cfg.get("period", "week")
    for k, d in (("per_hashtag", 60), ("details", 60), ("top", 30), ("music_top", 15), ("track_channels", 10), ("pause", 2.0)):
        cfg.setdefault(k, d)
    if args.details:
        cfg["details"] = args.details
    if args.cookies_from_browser:
        YTDLP.extend(["--cookies-from-browser", args.cookies_from_browser])
    patterns = [(w, re.compile(r"\b" + r"\s+".join(map(re.escape, w.split())) + r"\b", re.I)) for w in cfg["filter"]["blocklist"]]
    now = dt.datetime.now(dt.timezone.utc)
    date = now.astimezone().strftime("%Y-%m-%d")
    notes: list[str] = []

    # 1. discovery from hashtag pages (cheap list requests)
    jobs = [(g, tag) for g, tags in cfg["groups"].items() for tag in tags]
    print(f"reading {len(jobs)} hashtag pages...", file=sys.stderr)
    groups_of: dict[str, set] = collections.defaultdict(set)
    flat_views: dict[str, int] = {}
    with ThreadPoolExecutor(3) as pool:
        pages = pool.map(lambda j: flat_list(f"https://www.youtube.com/hashtag/{j[1]}/shorts", cfg["per_hashtag"]), jobs)
        for (group, _), entries in zip(jobs, pages):
            for e in entries:
                if e.get("id"):
                    groups_of[e["id"]].add(group)
                    flat_views[e["id"]] = max(flat_views.get(e["id"], 0), e.get("view_count") or 0)
    # Details for the most-viewed candidates of every group (round robin, so small groups count too).
    by_group = {g: sorted([i for i in groups_of if g in groups_of[i]], key=lambda i: -flat_views[i]) for g in cfg["groups"]}
    pick: list[str] = []
    while len(pick) < min(cfg["details"], len(groups_of)):
        for g in by_group:
            while by_group[g] and by_group[g][0] in pick:
                by_group[g].pop(0)
            if by_group[g] and len(pick) < cfg["details"]:
                pick.append(by_group[g].pop(0))

    # 1b. fresh uploads via API search: hashtag pages favour older hits
    key = os.environ.get("YOUTUBE_API_KEY")
    from_search: set[str] = set()
    if key and cfg.get("search"):
        after = now - dt.timedelta(days=PERIOD_DAYS[cfg["period"]])
        sjobs = [(g, q) for g, qs in cfg["search"].items() for q in qs]
        print(f"API search: {len(sjobs)} queries (~{100 * len(sjobs)} quota units)...", file=sys.stderr)
        for g, q in sjobs:
            try:
                ids = search_api(q, key, after)
            except urllib.error.HTTPError as e:
                notes.append(f"API search failed ({e.code}) for {q!r}; quota used up?")
                break
            for i in ids:
                groups_of[i].add(g)
                if i not in pick:
                    from_search.add(i)
        pick += sorted(from_search)

    # 2. details
    if key:
        mode = "YouTube Data API"
        metas = details_api(pick, key)
        # Search results include normal (landscape) videos under 4 min: keep only real Shorts.
        cand = [v for v in metas if v["id"] in from_search]
        with ThreadPoolExecutor(6) as pool:
            ok = {v["id"] for v, short in zip(cand, pool.map(lambda v: (v.get("duration") or 999) <= 180 and is_short(v["id"]), cand)) if short}
        metas = [v for v in metas if v["id"] not in from_search or v["id"] in ok]
    else:
        mode = "yt-dlp, slow mode (no YOUTUBE_API_KEY)"
        print(f"  {len(groups_of)} Shorts, details for {len(pick)} ({mode})...", file=sys.stderr)
        metas = details_ytdlp(pick, cfg["pause"], notes)

    # Whatever has no details (bot check, private) still gets title + channel from oEmbed.
    have = {v["id"] for v in metas}
    rest = [i for i in pick if i not in have and i not in from_search]
    with ThreadPoolExecutor(4) as pool:
        undated = [u | {"view_count": flat_views[u["id"]]} for u in pool.map(oembed, rest) if u]

    recent, older, excluded = [], [], []
    for v in undated[:]:
        v["groups"] = sorted(groups_of.get(v["id"], []))
        reason = blocked(v, patterns)
        if reason:
            excluded.append((v["id"], reason))
            undated.remove(v)
    undated.sort(key=lambda v: -v["view_count"])
    for v in metas:
        v["groups"] = sorted(groups_of.get(v["id"], []))
        reason = blocked(v, patterns)
        if reason:
            excluded.append((v["id"], reason))
            continue
        v["age"] = age_days(v, now) or 365.0
        v["vpd"] = (v.get("view_count") or 0) / v["age"]
        (recent if v["age"] <= PERIOD_DAYS[cfg["period"]] + 0.5 else older).append(v)
    recent.sort(key=lambda v: -v["vpd"])
    older.sort(key=lambda v: -v["vpd"])

    # 3. channel outliers
    channels = list(cfg.get("channels", {}).get("always", []))
    for v in recent + older + undated:
        if len(channels) >= cfg["track_channels"] + len(cfg.get("channels", {}).get("always", [])):
            break
        if v.get("channel_url") and v["channel_url"] not in channels:
            channels.append(v["channel_url"])
    print(f"  scanning {len(channels)} channels...", file=sys.stderr)

    def channel_tab(ch: str) -> list[dict]:
        base = ch if ch.startswith("http") else f"https://www.youtube.com/{ch}"
        return flat_list(base.rstrip("/") + "/shorts", 30)

    names = {v.get("channel_url"): v.get("channel") for v in recent + older + undated}
    outliers = []
    with ThreadPoolExecutor(3) as pool:
        for ch, entries in zip(channels, pool.map(channel_tab, channels)):
            for e in entries:
                e["channel"] = e.get("channel") or names.get(ch) or ch
            views = [e["view_count"] for e in entries if e.get("view_count")]
            if len(views) < 6:
                continue
            med = statistics.median(views)
            for e in entries[:10]:  # newest first
                if e.get("view_count") and e["view_count"] >= 3 * med and not blocked(e, patterns):
                    outliers.append({**e, "ratio": e["view_count"] / med})
    outliers.sort(key=lambda o: -o["ratio"])

    # 4. sounds of the top recent videos (plus top evergreen ones to fill up)
    targets = (recent + older)[: cfg["music_top"]] if len(recent) < cfg["music_top"] else recent[: cfg["music_top"]]
    music = {} if args.no_music else identify_music(targets, cfg["pause"], notes)

    out = Path(args.out) / date
    out.mkdir(parents=True, exist_ok=True)
    ctx = {"date": date, "cfg": cfg, "candidates": len(groups_of), "detailed": len(metas), "mode": mode, "notes": notes,
           "recent": recent, "older": older, "undated": undated, "excluded": excluded, "outliers": outliers, "music": music}
    write_report(out / "report.md", ctx)
    keep = ("id", "title", "channel", "view_count", "like_count", "comment_count", "duration", "age", "vpd", "groups", "tags", "made_for_kids")
    slim = lambda vs: [{k: v.get(k) for k in keep} | {"sound": music.get(v["id"])} for v in vs]
    (out / "trends.json").write_text(json.dumps(
        {"date": date, "period": cfg["period"], "recent": slim(recent), "older": slim(older), "undated": undated, "outliers": outliers, "excluded": excluded, "notes": notes},
        indent=1, ensure_ascii=False))
    print(out / "report.md")


if __name__ == "__main__":
    main()
