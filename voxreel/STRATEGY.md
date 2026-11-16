# Bananini Games: cele kanału i oczekiwana jakość

Dokument ustaleń z właścicielem kanału (pierwsza wersja 2026-11-16). Opisuje, co robimy, dla kogo, jak ma to
wyglądać i po czym poznajemy, że odcinek jest gotowy. Zmieniaj go, gdy zmieniają się ustalenia. Szczegóły
techniczne (komendy, formaty plików) są w [README.md](README.md).

---

## 1. Po co to robimy

**Cel biznesowy:** kanał z krótkimi animacjami (YouTube Shorts, potem TikTok i Reels), który regularnie zbiera duże
zasięgi wśród dzieci i kieruje ruch na [bananini.games](https://bananini.games) (buildy do pobrania dla Minecrafta,
gry na Robloxie).

**Widz:** dziecko w wieku mniej więcej 6–12 lat, które zna Minecrafta i ogląda Shorty na telefonie, często bez
dźwięku albo jednym okiem. Film musi być zrozumiały bez czytania i bez słuchania.

**Jak chcemy wygrywać:**
1. **Powtarzalność.** Jedna maszynka (voxreel) produkuje wiele odcinków w tym samym formacie. Nowy odcinek to nowy
   temat i scenariusz, nie nowy program. Celem jest odcinek w kilka godzin pracy, a nie w kilka dni.
2. **Jakość wyraźnie powyżej przeciętnego Shorta z Minecrafta.** Czytelna historia, ładny świat, kamera prowadzona
   jak przez operatora.
3. **Dopasowanie do trendów.** Format, długość, hook i dźwięk dobieramy na podstawie researchu
   (`mise run trends`), a nie na wyczucie.

**Mierniki sukcesu** (sprawdzamy po publikacji):
- wyświetlenia na dzień w pierwszym tygodniu, porównane z najlepszymi filmami z raportu trendów w tej samej kategorii;
- odsetek widzów, którzy oglądają do końca i zapętlają film (YouTube Studio: "obejrzane w całości" i "przesunięte dalej");
- ruch na bananini.games z opisu i komentarzy.

## 2. Główny format: "How It's Made" (wersja story)

Wybrany po raporcie trendów z 2026-11-16. Filmy typu "How X Are Made" (np. Usuko) miały do 1M wyświetleń dziennie
i są przyjazne dzieciom.

**Na czym polega:** klockowe postacie (Steve, Alex) w świecie w stylu Minecrafta pokazują krok po kroku, jak
powstaje coś, co dziecko zna i lubi: pizza, lody, miód, chleb, czekolada, kredki... Historia jest **ciągła**:
postacie chodzą między stanowiskami, noszą składniki, pracują przy stole i piecu, a na końcu cieszą się z efektu.

**Budowa odcinka:**

| część | czas | co widać |
|---|---|---|
| Hook | ~1,5 s | gotowy produkt i postacie, które się cieszą, nad nimi pytanie "HOW IS PIZZA MADE?" |
| Kroki 1–4 | ~5–7 s każdy | "STEP n" + krótki podpis; postacie wykonują czynność od początku do końca |
| Finał | ~3–4 s | "TA-DA!", gotowy produkt, postacie wiwatują, fanfara |
| Pętla | – | ostatnie ujęcie przechodzi w hook, więc film zapętla się bez szwu |

**Długość:** cel 20–30 s. Obecna pizza ma 33 s, więc kolejne odcinki trzeba skracać, chyba że dane pokażą, że
dłuższe działają lepiej.

**Odcinki:** pizza (wersja story, punkt odniesienia jakości), lody i miód (na razie w starym formacie dioram,
do przepisania). Kolejne tematy wybieramy z raportu trendów i z tego, co dzieci znają z kuchni i z Minecrafta.

## 3. Oczekiwana jakość

Punktem odniesienia jest **odcinek o pizzy w wersji story** (`story/pizza`, `out/story/pizza.mp4`). Właściciel
kanału ocenił go jako "już bardzo blisko quality, które chcemy osiągnąć". Każdy nowy odcinek ma być co najmniej
tak dobry. Poniższe zasady wynikają z konkretnych uwag do wcześniejszych wersji.

### 3.1 Świat i proporcje

- **Skala jak w Minecrafcie:** postać ma 2 bloki wysokości (1 blok = 9,6 voxela). Stół ma wysokość 1 bloku, dom
  5–8 bloków, drzewo 5–7 bloków. *(Uwaga do wersji 1: "środowisko wydaje się za małe w proporcjach do ludzików".)*
- **Prawdziwe miejsce, a nie diorama na pustym tle:** plac, wioska, farma albo łąka z domami, drzewami, ścieżkami
  i żywopłotami w tle. *(Uwaga: "tło jest słabe".)*
- Świat budujemy z bloków z delikatną pikselową teksturą. Rekwizyty (jedzenie, naczynia, rośliny) są drobniejsze
  i szczegółowe, żeby było jasne, co się dzieje.
- Każde stanowisko (ogródek, stół, piec) ma czytelną sylwetkę z odległości kamery.

### 3.2 Postacie i scenariusz

- **Jedna ciągła historia,** a nie scenki pojawiające się jedna po drugiej. Rzeczy przechodzą z miejsca na
  miejsce i z rąk do rąk: pomidory z krzaka do koszyka i na stół, pizza ze stołu do pieca i z powrotem.
  *(Uwaga: "brakuje skryptu, że chodzą i noszą do pieca... cała animacja, a nie spawn jedno po drugim".)*
- Każdy krok ma widoczną przyczynę i skutek: postać miesza, a ciasto rośnie w misce; kroi, a sos pojawia się na
  cieście; wkłada pizzę do pieca, a wyjmuje ją upieczoną.
- **Zero przenikania:** postacie nie przechodzą przez siebie ani przez stoły, piec i ściany. Skrypt omija
  przeszkody, a `s.check()` nie może zgłaszać problemów przed renderem. *(Uwaga: "przechodzą przez siebie ludziki".)*
- Postacie są skierowane do akcji albo do kamery, nie stoją do widza plecami w kluczowym momencie.
- Ruch jest żywy: chód z ruchem rąk, gesty pasujące do czynności (mieszanie, ugniatanie, krojenie, zrywanie,
  wkładanie do pieca), radość na końcu.
- Postacie z Minecrafta (Steve, Alex) są w porządku (patrz punkt 5). Stroje i czapki dopasowujemy do roli: kucharz,
  farmer, pszczelarz.

### 3.3 Kamera

- **Kamera jak operator:** w każdym kroku ma ujęcie, które pokazuje akcję. Podąża za postacią, gdy ta idzie, i obejmuje
  stanowisko, gdy postać pracuje. Między krokami są cięcia, a w trakcie ujęcia płynny ruch. *(Uwaga: "operator kamery".)*
- **Nic nie zasłania akcji:** żadna postać ani obiekt między kamerą a tym, co ważne. Kamera nie wchodzi w ściany,
  dachy ani drzewa i nie ma dużych obiektów tuż przed obiektywem. *(Uwaga: "kamerka do poprawy".)*
- Kadr pionowy 9:16: akcja w środkowej części ekranu. Górne ~25% zajmuje tytuł, a dolne ~20% zasłania interfejs
  YouTube, więc nic ważnego nie może tam być.
- Kamera nie stoi w miejscu: zawsze jest lekki ruch (dolly, orbita), ale bez szarpnięć.

### 3.4 Dźwięk

- **W pliku nie ma cudzej muzyki** (patrz punkt 4). W filmie jest pełny, dopracowany podkład efektów:
  - tło miejsca: ptaki i wiatr, a przy miodzie pszczoły;
  - kroki postaci;
  - dźwięki czynności zsynchronizowane z ruchem rąk: mieszanie, ugniatanie, stukanie noża, zrywanie, nalewanie, chrupanie;
  - stuk układanych bloków;
  - "bloop" przy podpisie, "whoosh" przy cięciu, pytająca melodyjka w hooku, fanfara "ta-da" na końcu.
- Poziomy wyrównane: żaden efekt nie zagłusza reszty, całość znormalizowana. *(Uwaga do wersji 1: "dźwięki są słabe".)*
- Głośność efektów zostawia miejsce na popularny dźwięk dodawany przy wrzucaniu.

### 3.5 Tekst na ekranie

- Duże, grube litery z obrysem, czytelne na telefonie: u góry BANANINI.GAMES, "HOW IT'S MADE" i tytuł, na dole
  "STEP n" i podpis.
- Podpis ma najwyżej 4–5 prostych angielskich słów, które dziecko zrozumie, nawet jeśli dopiero uczy się czytać.
- Bez wzmianek o AI i bez nazw modeli.

### 3.6 Technicznie

- 1080×1920, 60 fps, H.264 + AAC (sprzętowy enkoder na Macu; `--encoder x264` daje maksymalną jakość).
- Render odcinka trwa około minuty lub mniej (pizza: 33 s filmu w 22 s), żeby poprawki dało się szybko sprawdzać.
- Każda zmiana silnika musi zachować działające stare formaty (Build Battle, diorama "made").

## 4. Muzyka i dźwięk z trendów

- **Nie wklejamy popularnych utworów do pliku mp4.** Content ID przejmie zarobki albo wyciszy lub zablokuje film.
- Popularny dźwięk dodajemy **przy wrzucaniu w aplikacji YouTube Shorts** ("Dodaj dźwięk"). Utwór jest wtedy
  licencjonowany, a film trafia do feedu tego dźwięku. Dźwięk dobieramy do konkretnego filmu na podstawie raportu
  trendów. Muzyka w niszy jest rozproszona, więc nie ma jednego hitu na całą serię.
- Lektor (jeśli kiedyś wróci): ElevenLabs eleven-v4, głos "will" (patrz README). Obecny format działa bez lektora.

## 5. Bezpieczeństwo i zasady treści

**Nie robimy:**
- Brainrotu: skibidi, włoskie memy (tung tung sahur, bombardiro, tralalero...), sigma/rizz/ohio/67, Sprunki.
- Rzeczy obrzydliwych i strasznych: tycie, toalety, horror, jumpscare'y, krew, przemoc, wątki romantyczne.
- Wzmianek o AI ani nazw modeli w filmach.
- Kopii postaci innych marek (np. JJ i Mikey z Maizen) ani cudzej muzyki w pliku.

**Minecraft:** postacie i content Minecrafta (Steve, Alex) są w porządku. Wytyczne Mojang (Minecraft Usage
Guidelines) pozwalają robić z nimi filmy, także monetyzowane. Nie sugerujemy, że to oficjalny produkt, i nie
używamy logo Minecrafta jako własnej marki. W opisie każdego filmu:
"Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft."

## 6. Jak powstaje odcinek

1. **Temat:** z raportu trendów (`mise run trends`) i listy rzeczy, które dzieci znają.
2. **Scenariusz:** 4 kroki z jasną przyczyną i skutkiem, kto co nosi i dokąd. Zapisany jako funkcja w
   `tools/story.py` (świat, stanowiska, rekwizyty, postacie, ujęcia kamery).
3. **Kontrola skryptu:** `uv run --no-project tools/story.py <odcinek>`. `s.check()` nie może nic zgłaszać.
4. **Podgląd klatek:** `--still <t> --height 960` dla każdego kroku. Sprawdzamy listę z punktu 7.
5. **Render:** `cargo run --release -- story story/<odcinek>/script.json`, a potem obejrzenie całości na telefonie,
   z dźwiękiem i bez.
6. **Publikacja:** tytuł i opis bez AI, z dopiskiem o Minecrafcie i linkiem do bananini.games. Dźwięk z trendów
   dodany w aplikacji.
7. **Po tygodniu:** wyniki porównane z raportem trendów i wnioski dopisane tutaj.

## 7. Lista kontrolna przed publikacją

- [ ] Hook w pierwszej sekundzie: widać efekt końcowy i pytanie.
- [ ] Każdy krok da się zrozumieć bez czytania podpisu.
- [ ] Rzeczy przechodzą z rąk do rąk i z miejsca na miejsce. Nic nie "wyskakuje" bez przyczyny (wyjątek: celowe
      układanie z bloków, które jest znakiem rozpoznawczym kanału).
- [ ] Nikt nie przenika przez postacie ani obiekty (`s.check()` czysty, na podglądzie też).
- [ ] W żadnej klatce kamera nie jest w ścianie ani dachu i nic nie zasłania akcji. Brak pleców postaci w kluczowym momencie.
- [ ] Akcja nie wchodzi pod podpisy ani pod interfejs YouTube.
- [ ] Proporcje jak w Minecrafcie: postać 2 bloki, domy i drzewa w tle wyraźnie większe.
- [ ] Dźwięki czynności pasują do ruchu. Tło miejsca słychać, ale nie dominuje. Fanfara na końcu.
- [ ] Koniec płynnie przechodzi w początek (pętla).
- [ ] 20–30 s (albo świadomie inaczej, z uzasadnieniem z danych).
- [ ] Zasady z punktu 5 spełnione, opis zawiera dopisek o Minecrafcie.

## 8. Historia formatów (żeby nie wracać do tego, co odrzucone)

- **Build Battle** (3 budowle, głosowanie 1/2/3): pierwszy format. Działa i zostaje w repo. Raport trendów pokazał,
  że mechanizm "1, 2 czy 3?" jest popularny, ale nasze filmy są za długie (~35 s, najlepsze mają 10–25 s).
- **"Made" v1** (`made/`, `tools/made.py`): osobne dioramy na każdy krok, budowane z bloków, potem z postaciami i tłem.
  Odrzucone jako docelowe, bo był to ciąg scenek zamiast historii, środowisko było za małe względem postaci,
  a dźwięk i tło były słabe. Kod zostaje, bo wykorzystuje go wersja story.
- **"Made" story** (`story/`, `tools/story.py`, `src/story.rs`): obecny format docelowy.
- Do sprawdzenia w przyszłości: generowanie terenu i ładne lokacje, chodzące maszyny (redstone, Create Mod). W raporcie
  z 2026-11-16 maszyny miały pojedyncze hity, ale słabą medianę.

## 9. Research trendów

- `mise run trends` (w `voxreel/`): YouTube Shorts z danego okresu (`--period day|week|month`), ranking wyświetleń
  na dzień, porównanie formatów (grupy hashtagów i zapytań), dźwięki rozpoznane Shazamem, filmy, które biją medianę
  swojego kanału, oraz filtr brainrotu. Raport: `research/out/<data>/report.md`.
- Konfiguracja (hashtagi, zapytania do API, śledzone kanały, lista blokowanych słów): `research/config.toml`.
- Klucz YouTube Data API jest w `research/.env` (`YOUTUBE_API_KEY`, nie trafia do gita). Jeden skan zużywa
  ok. 1000 z 10 000 darmowych jednostek dziennie.
- TikTok: brak publicznego API do list trendów (Creative Center wymaga logowania, a yt-dlp nie obsługuje już
  hashtagów i dźwięków). Na razie sprawdzamy go ręcznie. Do automatu potrzebny jest oficjalny dostęp albo płatne API.
- Wzorce z raportu z 2026-11-16 (do sprawdzenia w kolejnych raportach):
  - "How X Are Made" (Usuko): do 1M wyświetleń dziennie, bez lektora;
  - "1, 2 or 3?" (Sir Potato): 4,6M dziennie;
  - timelapse budowania z zabawnym podpisem (NecronCore);
  - postacie Maizen (JJ i Mikey) są popularne, ale to cudza marka i często straszne motywy, więc ich nie robimy.
