"""Upload a finished short to the Bananini Games YouTube channel.

    mise run upload -- --login                                  # one-time browser login
    mise run upload -- ../out/story/pizza.mp4 videos/pizza.json  # private upload (asks first)
    mise run upload -- ../out/story/pizza.mp4 videos/pizza.json --publish-at 2026-11-20T16:00:00+01:00
    mise run upload -- ../out/story/pizza.mp4 videos/pizza.json --dry-run

Safety defaults: private, "made for kids" set (COPPA: the channel's audience is children), and a
confirmation prompt before anything is sent (`--yes` skips it). Videos from an unverified Google
Cloud project stay private no matter what is requested, until the project passes YouTube's audit;
publish them by hand in YouTube Studio.

Needs `client_secret.json` (OAuth "Desktop app" client) next to this file; the login token is
cached in `.token.json`. Both are git-ignored. Setup: see README "Publishing".
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CLIENT = HERE / "client_secret.json"
TOKEN = HERE / ".token.json"
SCOPES = ["https://www.googleapis.com/auth/youtube.upload", "https://www.googleapis.com/auth/youtube.readonly"]

DISCLAIMER = "Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft."
FOOTER = "Get the builds for Minecraft and play our games: https://bananini.games"
CATEGORY_GAMING = "20"


def credentials():
    from google.auth.transport.requests import Request
    from google.oauth2.credentials import Credentials
    from google_auth_oauthlib.flow import InstalledAppFlow

    creds = Credentials.from_authorized_user_file(str(TOKEN), SCOPES) if TOKEN.exists() else None
    if creds and creds.valid:
        return creds
    if creds and creds.expired and creds.refresh_token:
        try:
            creds.refresh(Request())
            TOKEN.write_text(creds.to_json())
            return creds
        except Exception as e:  # revoked or expired (Testing apps: 7 days) -> log in again
            print(f"token refresh failed ({e}), logging in again", file=sys.stderr)
    if not CLIENT.exists():
        sys.exit(f"missing {CLIENT}\nCreate an OAuth 'Desktop app' client and save its JSON there (README: Publishing).")
    flow = InstalledAppFlow.from_client_secrets_file(str(CLIENT), SCOPES)
    creds = flow.run_local_server(port=0, prompt="consent")
    TOKEN.write_text(creds.to_json())
    TOKEN.chmod(0o600)
    return creds


def youtube():
    from googleapiclient.discovery import build

    return build("youtube", "v3", credentials=credentials(), cache_discovery=False)


def channel_name(yt) -> str:
    items = yt.channels().list(part="snippet", mine=True).execute().get("items", [])
    return items[0]["snippet"]["title"] if items else "(no channel on this account)"


def body_from(meta: dict, privacy: str, publish_at: str | None, kids: bool) -> dict:
    tags = meta.get("tags", [])
    description = "\n\n".join(p for p in [meta.get("description", "").strip(), FOOTER, DISCLAIMER] if p)
    hashtags = " ".join(f"#{h.lstrip('#')}" for h in meta.get("hashtags", ["shorts", "minecraft"]))
    if hashtags:
        description += "\n\n" + hashtags
    status = {"privacyStatus": privacy, "selfDeclaredMadeForKids": kids, "embeddable": True}
    if publish_at:
        # YouTube publishes a scheduled video itself; it has to be private until then.
        when = dt.datetime.fromisoformat(publish_at)
        if when.tzinfo is None:
            sys.exit("--publish-at needs a timezone, e.g. 2026-11-20T16:00:00+01:00")
        status["privacyStatus"] = "private"
        status["publishAt"] = when.astimezone(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    title = meta["title"]
    if len(title) > 100:
        sys.exit(f"title is {len(title)} characters, YouTube allows 100")
    return {
        "snippet": {
            "title": title,
            "description": description,
            "tags": tags,
            "categoryId": meta.get("category", CATEGORY_GAMING),
            "defaultLanguage": meta.get("language", "en"),
            "defaultAudioLanguage": meta.get("language", "en"),
        },
        "status": status,
    }


def upload(yt, video: Path, body: dict):
    from googleapiclient.http import MediaFileUpload

    media = MediaFileUpload(str(video), mimetype="video/mp4", chunksize=8 * 1024 * 1024, resumable=True)
    req = yt.videos().insert(part="snippet,status", body=body, media_body=media)
    resp = None
    while resp is None:
        status, resp = req.next_chunk()
        if status:
            print(f"\r  uploading {status.progress() * 100:5.1f}%", end="", file=sys.stderr)
    print(file=sys.stderr)
    return resp


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("video", nargs="?", type=Path)
    ap.add_argument("meta", nargs="?", type=Path, help="videos/<name>.json: title, description, tags, hashtags")
    ap.add_argument("--privacy", choices=["private", "unlisted", "public"], default="private")
    ap.add_argument("--publish-at", help="schedule (ISO 8601 with timezone); the video stays private until then")
    ap.add_argument("--not-for-kids", action="store_true", help="don't mark as made for kids (only if it really isn't)")
    ap.add_argument("--dry-run", action="store_true", help="print what would be sent, send nothing")
    ap.add_argument("--yes", action="store_true", help="skip the confirmation prompt")
    ap.add_argument("--login", action="store_true", help="log in (or switch account) and show the channel")
    args = ap.parse_args()

    if args.login:
        if TOKEN.exists():
            TOKEN.unlink()
        print("channel:", channel_name(youtube()))
        return
    if not args.video or not args.meta:
        ap.error("pass the video file and its metadata json (or --login)")
    if not args.video.exists():
        sys.exit(f"no such video: {args.video}")
    meta = json.loads(args.meta.read_text())
    body = body_from(meta, args.privacy, args.publish_at, kids=not args.not_for_kids)

    print(json.dumps(body, indent=2, ensure_ascii=False))
    print(f"video: {args.video} ({args.video.stat().st_size / 1e6:.1f} MB)")
    if args.dry_run:
        return
    yt = youtube()
    print(f"channel: {channel_name(yt)}")
    if not args.yes and input("Upload to this channel? [y/N] ").strip().lower() != "y":
        print("cancelled")
        return
    resp = upload(yt, args.video, body)
    vid = resp["id"]
    got = resp.get("status", {}).get("privacyStatus")
    print(f"uploaded: https://youtube.com/shorts/{vid}  (privacy: {got})")
    print(f"studio:   https://studio.youtube.com/video/{vid}/edit")
    if args.privacy != "private" and got == "private":
        print("note: YouTube kept it private (unverified API project); publish it in Studio.")


if __name__ == "__main__":
    main()
