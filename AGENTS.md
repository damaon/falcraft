# Repository Guidelines

## Project Structure & Module Organization
Falcraft is a Fabric mod for Minecraft 1.21.1 with split source sets:

- `src/main/java/com/falcraft/`: common mod entrypoints and shared logic
- `src/client/java/com/falcraft/`: client-only features (commands, rendering, voxel/GLB utilities)
- `src/main/resources/`: mod metadata (`fabric.mod.json`, mixin config, assets)
- `src/client/resources/`: client mixin config
- `.github/workflows/build.yml`: CI build pipeline (`./gradlew build`)

Keep related code in focused classes (one class/function, one responsibility) and avoid oversized files.

## Build, Test, and Development Commands
- `./gradlew runClient`: start Minecraft dev client with the mod loaded.
- `./gradlew build`: compile and package the mod JAR (`build/libs/`), runs CI-equivalent checks.
- `./gradlew clean build`: full rebuild from scratch.
- `./gradlew tasks`: list available Gradle tasks when adding workflows.

Proper compile flow (local machine):
- Require Java 21+ (`java -version`). Java 20 or lower will fail with Fabric Loom dependency errors.
- Prefer local Gradle cache inside repo: `GRADLE_USER_HOME=.gradle ./gradlew build`
- If Java 21 is not installed system-wide, use a project-local OpenJDK:
  - `mkdir -p .tooling && cd .tooling`
  - `curl -L -o jdk21.tar.gz "https://api.adoptium.net/v3/binary/latest/21/ga/mac/aarch64/jdk/hotspot/normal/eclipse"`
  - `tar -xzf jdk21.tar.gz`
  - `cd ..`
  - `JAVA_HOME="$(ls -d "$PWD"/.tooling/jdk-21*/Contents/Home | head -n 1)" PATH="$JAVA_HOME/bin:$PATH" GRADLE_USER_HOME=.gradle ./gradlew build`
- Output JAR: `build/libs/falcraft-<version>.jar`

Environment setup for API calls:
- Create `run/.env` with `FAL_API_KEY=...` for local development.

## Coding Style & Naming Conventions
- Java 21 target (`sourceCompatibility`/`targetCompatibility` set to 21).
- Indentation: 4 spaces, no tabs.
- Packages: lowercase (`com.falcraft...`).
- Classes: PascalCase (`GenerateCommand`, `GhostBlockRenderer`).
- Methods/fields: camelCase.
- Prefer descriptive names and comments that explain **why**, not **what**.
- Use SLF4J logging with contextual messages for network/IO paths.

## Testing Guidelines
There is currently no dedicated automated test suite in `src/test`. Minimum validation for changes:

- `./gradlew build` passes locally.
- Behavior is verified in `./gradlew runClient` (command flows, preview/placement, error handling).
- For regressions, document repro steps in PR and test against at least one invalid-input path.

## Commit & Pull Request Guidelines
Git history shows mixed styles, with growing use of Conventional Commits (`feat:`, `refactor:`, `docs:`). Prefer:

- `type(scope): concise summary` (example: `feat(stream): add cancel feedback`).
- One logical change per commit.
- Reference issues in commit/PR text (`Fixes #123`) when applicable.

PRs should include:
- What changed and why.
- How it was tested (`build`, in-game checks).
- Screenshots/log snippets for user-visible or runtime-behavior changes.

## Security & Configuration Tips
- Never commit `.env` or API keys.
- Rotate keys immediately if exposed.
- Use timeouts and explicit error handling for external API calls.
