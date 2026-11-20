# Run with Docker

For a NAS or any machine that already runs Docker. The image holds the one
binary and nothing else of the server's: its database and covers go to a
folder you choose, and your comics are handed to it read-only.

There is no published image yet. You build it from this repository and carry
it over as a file.

## 1. Build the image

On a machine with this repository and Docker:

```sh
BINGE_PLATFORM=linux/amd64 scripts/docker.sh
```

Use `linux/amd64` for a NAS with an Intel or AMD processor and `linux/arm64`
for one with an ARM processor. Without `BINGE_PLATFORM` the image is for the
machine that builds it.

The script prints the file it saved, such as
`target/docker/binge-server-0.1.0-amd64.tar.gz`.

## 2. Load it where it will run

Copy the file there, then:

```sh
docker load -i binge-server-0.1.0-amd64.tar.gz
```

On a Synology NAS, Container Manager does the same: **Image**, **Action**,
**Import**, **Add from file**.

## 3. Choose two kinds of folder

- **One folder for the server's data.** Its database, with the accounts and
  everyone's reading progress, and the covers it makes. Keep it; back it up.
- **Your comic folders.** As many as you like. Each is mounted with `:ro`,
  read-only, so the server cannot change a file in it. That is Docker's
  doing, on top of the server's own rule never to write to a library.

## 4. Start it

With Docker Compose, in a file named `compose.yaml`:

```yaml
services:
  binge-server:
    image: binge-server:latest
    container_name: binge-server
    restart: unless-stopped
    ports:
      - "8686:8686"
    volumes:
      # The server's data: left side is a folder on your machine.
      - /volume1/docker/binge-server:/data
      # Your comics, read-only. Add a line for each folder.
      - /volume1/comics:/comics:ro
```

```sh
docker compose up -d
```

On a Synology NAS: **Project**, **Create**, pick a folder, and paste the
file.

Or with one command:

```sh
docker run -d --name binge-server --restart unless-stopped \
  -p 8686:8686 \
  -v /volume1/docker/binge-server:/data \
  -v /volume1/comics:/comics:ro \
  binge-server:latest
```

## 5. Make the owner, then add libraries

Open `http://<the machine's address>:8686` and make the owner account
**right away**. Until it has an owner, the server belongs to whoever opens
it first.

Then add a library. The folder to type is the one **inside the container**,
the right side of the mount: `/comics`, or a folder below it.

## Settings

The image sets these; change them with `environment:` in the Compose file.

| Variable | In the image | What it does |
| --- | --- | --- |
| `BINGE_DATA_DIR` | `/data` | Where the server writes |
| `BINGE_BIND` | `0.0.0.0` | Listens on the container's network, which Docker publishes |
| `BINGE_PORT` | `8686` | The port inside the container |
| `BINGE_ALLOWED_HOSTS` | not set | Host names the server answers to, such as `nas.local`. IP addresses always work |
| `BINGE_NO_UPDATES` | not set | Set to `true` and the server never looks for a newer version and never starts one it fetched: the image decides the version |

## Updating and removing

To update, the owner opens **Settings**, **About** in the web app and
presses **Look for a new version**, then **Update**. The server fetches
the new version into `updates/` in the data folder, makes sure Binge
signed it, and starts again as it. The container and the image stay as
they are: each time the container starts, the image's server runs the
newest version in the data folder in its place. So the version that runs
can be newer than the image's tag; the About page says which runs.

A new image works too: load it and recreate the container with
`docker compose up -d`. When the image is newer than what the data folder
holds, the image's version runs. Either way the data folder stays, and a
new version brings its database up to date when it starts.

Do not delete `updates/` from the data folder after an update. The
database is then newer than the image, and the image's server cannot open
it.

If the server does not come back after an update, load an image of a
newer version and recreate the container. An image newer than what the
data folder holds runs in its place.

To remove, `docker compose down` and delete the data folder. Nothing was
ever written to your comics.

## What to know

- **It runs as root inside the container**, so it can read your comics
  whoever owns them on the NAS. With the comics mounted read-only it still
  cannot change them. To run it as another user, add `user: "1026:100"`
  (your user and group numbers) and make sure that user can read the comics
  and write the data folder.
- **No TLS.** The server speaks plain HTTP. Keep it on your home network, or
  put a TLS proxy in front of it.
- **Memory.** A PDF is read whole when one of its pages is asked for. On a
  NAS with little memory, very large PDFs are the thing to watch.
- **The image checks its own health.** Every 30 seconds the container
  runs `binge-server health`, which asks the server inside it and fails
  when it does not answer or its database does not. `docker ps` and
  Container Manager show the result as healthy or unhealthy. Docker only
  reports it: it does not restart an unhealthy container by itself.
