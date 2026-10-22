# DriveSync build profiles

The same Rust application is built for two distinct Flatpak IDs:

| Environment | Flatpak manifest | Application ID | OAuth client |
| --- | --- | --- | --- |
| Development / GNOME Builder | `flatpak/com.rtosta.DriveSync.Devel.yml` | `com.rtosta.DriveSync.Devel` | `data/oauth/development.json` (optional) |
| Production candidate | `flatpak/com.rtosta.DriveSync.yml` | `com.rtosta.DriveSync` | `data/oauth/production.json` (required) |

Both profiles compile the same binary and reuse the same source icon,
desktop entry and metainfo templates. The production manifest installs
desktop integration under the production app ID.

## Google OAuth

Create **separate Google Cloud projects** for Testing and Production, each
with a Desktop OAuth client. Download the client JSON from each project into
the corresponding ignored file under `data/oauth/`. **Never commit refresh
tokens or individual user credentials.** The desktop client identifier is
public metadata; the optional Desktop client secret is not a confidential
secret in a distributed desktop executable.

The GNOME Builder development manifest builds without its JSON, for UI-only
testing, but Google login remains unavailable until a valid client file is
provided and the Google account is in that OAuth project's test-user list.

The production manifest fails its build if its JSON is missing or invalid.
A production client must be configured and Google verification requirements
must be satisfied before public distribution. The production manifest is a
**candidate**, not yet a Flathub-ready offline/reproducible build: Cargo
dependencies must be provided through an appropriate Flatpak sources flow.

For release CI, provision `data/oauth/production.json` in the build job
from the release configuration before running Flatpak Builder. GitHub
Actions secrets are **not automatically available to Flathub builders**.
For a reproducible Flathub package, embed or stage the non-confidential
production Desktop OAuth client metadata from reviewable packaging sources.

Changing application ID changes the desktop identity; users may need to
authorize the other OAuth client again. Neither build shares OAuth tokens
with the other through the build artifacts.

The engine's build script also supports overriding the OAuth path using
`DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE`, for custom local builds.
