# Human-capacity marketing reframe — implementation review

Date: 2026-07-22 (Pacific/Auckland)

Result: **PASS**. The rendered pages passed the route/content contract, browser-boundary checks,
desktop/mobile visual review, contrast review, forced-colors and no-CSS checks, three isolated
first-read reviews, and one isolated site-wide semantic claim audit.

## Production deployment

Deployed on 2026-07-22 with Wrangler as Cloudflare Workers version
`6b6453bc-57e3-4d98-be29-403d725a8b17`. Post-deploy verification covered both
`https://riot-protest-net-marketing.protestnet.workers.dev` and `https://riot.protest.net`.
At both origins, all nine editorial routes returned HTTP 200 with bytes identical to their committed
`marketing/public/` files and no `Set-Cookie` response header. An unknown route returned HTTP 404.

## Rendered-file identity

The three first-read sessions reviewed the same byte-for-byte file:

```text
b209f0073f39d4feee8ebb33fc681d2c002748f607004eaac371c2b75bf77ce9  marketing/public/why-riot/index.html
```

## Visual evidence

Playwright rendered the committed public mirror from a loopback-only static preview. All six
standard captures had no horizontal overflow. Screenshot files are reproducible temporary review
artifacts, not site assets.

Repeat the standard, forced-colors, no-CSS, and overflow captures from the repository root with:

```sh
set -eu
evidence=/tmp/visual-review/riot-human-capacity
mkdir -p "$evidence"
python3 -m http.server 4173 --bind 127.0.0.1 --directory marketing/public >"$evidence/server.log" 2>&1 &
server_pid=$!
trap 'kill "$server_pid" 2>/dev/null || true' EXIT HUP INT TERM
export RIOT_VISUAL_ORIGIN=http://127.0.0.1:4173/
until curl --fail --silent --output /dev/null "$RIOT_VISUAL_ORIGIN"; do sleep 0.1; done
node --input-type=module <<'NODE'
import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
import { mkdir } from "node:fs/promises";

const origin = process.env.RIOT_VISUAL_ORIGIN;
const evidence = "/tmp/visual-review/riot-human-capacity";
const browser = await chromium.launch({ headless: true });
await mkdir(evidence, { recursive: true });
const captures = [["home", "/"], ["why-riot", "/why-riot/"], ["privacy", "/privacy/"]];
for (const [size, viewport] of [["desktop", { width: 1456, height: 900 }], ["mobile", { width: 390, height: 844 }]]) {
  for (const [name, route] of captures) {
    const page = await browser.newPage({ viewport, reducedMotion: "reduce" });
    await page.goto(new URL(route, origin).href, { waitUntil: "networkidle" });
    const dimensions = await page.evaluate(() => ({ scrollWidth: document.documentElement.scrollWidth, clientWidth: document.documentElement.clientWidth }));
    assert.ok(dimensions.scrollWidth <= dimensions.clientWidth, `${name}-${size} horizontal overflow`);
    await page.screenshot({ path: `${evidence}/${name}-${size}.png`, animations: "disabled" });
    await page.close();
  }
}
for (const [name, route] of captures.slice(1)) {
  const context = await browser.newContext({ viewport: { width: 1456, height: 900 }, forcedColors: "active", reducedMotion: "reduce" });
  const page = await context.newPage();
  await page.goto(new URL(route, origin).href, { waitUntil: "networkidle" });
  await page.screenshot({ path: `${evidence}/${name}-forced-colors.png`, animations: "disabled" });
  await context.close();
}
const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
await page.goto(new URL("/why-riot/", origin).href, { waitUntil: "networkidle" });
await page.evaluate(() => document.querySelectorAll("style,link[rel='stylesheet']").forEach((element) => element.remove()));
const noCss = await page.evaluate(() => ({ scrollWidth: document.documentElement.scrollWidth, clientWidth: document.documentElement.clientWidth }));
assert.ok(noCss.scrollWidth <= noCss.clientWidth, "why-riot-no-css horizontal overflow");
await page.screenshot({ path: `${evidence}/why-riot-no-css.png`, animations: "disabled" });
await browser.close();
NODE
npm run test:marketing
shasum -a 256 "$evidence"/*.png "$evidence/browser-evidence.json"
```

| Capture | Viewport | SHA-256 |
|---|---:|---|
| `/tmp/visual-review/riot-human-capacity/home-desktop.png` | 1456×900 | `9c85b2540b3614fcd662373858630319d4c731a7694bdbf5546b96157a4b5b9d` |
| `/tmp/visual-review/riot-human-capacity/home-mobile.png` | 390×844 | `2783461c2dca678d40088af451c28cbf91ef1a226c2939cb5312c46e1d00e1cc` |
| `/tmp/visual-review/riot-human-capacity/why-riot-desktop.png` | 1456×900 | `9979d814ba6b2906f69a5b4df734f0abf430d18f22dfe0cdb82a48d7386d1b7b` |
| `/tmp/visual-review/riot-human-capacity/why-riot-mobile.png` | 390×844 | `fc4b8f65ab98e8901528f5a41337493a90e04e5ad80b972d7ee2353c03910682` |
| `/tmp/visual-review/riot-human-capacity/privacy-desktop.png` | 1456×900 | `b151ec571c9906e73b034df1532dc981c85594727d24d2b4de1a5f9b54c0ce26` |
| `/tmp/visual-review/riot-human-capacity/privacy-mobile.png` | 390×844 | `08893494f42708d9643f85adcbda0d3339da8d377987a977c663ddd202f545dc` |

Additional checks:

| Check | Result | SHA-256 |
|---|---|---|
| Why Riot forced colors, 1456×900 | usable; no overflow | `afdc3d5a9a59aee39def580ea6f3b821b5aa2172e8dc80579d70cf5a95b38a35` |
| Privacy forced colors, 1456×900 | usable; no overflow | `738acbaa9228b9da1fcbf6d7caa0e4f90a62f466f62b7c2b59bde7d92f56ae77` |
| Why Riot without CSS, 390×844 | no overflow; SVG precedes Tools section | `d765af3ddac6ac9dd8b9495999adbcfe776a2021cd84b7cd34d4cdb26bd06848` |
| Complete visual evidence JSON | recorded | `c46adbd8c32494f08aeac3bc2e2a29716e3e9596d19c9fce4e0ec027c3d4b3df` |

Contrast evaluation inspected every visible text node against the nearest flat background at both
viewports. Results were: Home 26 unique pairs and zero failures at each viewport; Why Riot 21
desktop/22 mobile pairs and zero failures; Privacy 13 desktop/14 mobile pairs and zero failures.
Home had eleven elements whose nearest background used a semi-transparent `color()` value the
flat-background parser deliberately leaves unresolved. They all sit in `.builder-card`, a 5% paper
overlay on `#17160f`. Manual compositing gives `#222019`; pink text is 6.28:1 and paper text is
13.06:1. The same elements and colors occur at both viewports.

Visual inspection confirmed that ordinary collective life is visually primary, technical mechanism
is subordinate, the existing desktop/mobile app-screen composition remains intact, the Privacy page
is a concise boundary reference, and no page presents a disaster scene as its central image.

## Browser and HTTP evidence

Evidence file:

```text
bbaa8f6b1dd879ce347aa742c0ded1c7ebfbfa6156464ae01963a860f7a9684d  /tmp/visual-review/riot-human-capacity/browser-evidence.json
```

Preview origin: `http://127.0.0.1:55130`. Each route began and ended with an empty Playwright
cookie jar, returned an empty `document.cookie`, and received no `Set-Cookie` response header.
Every observed request used that exact loopback origin. `/` requested only itself and the six local
`/assets/screenshots/*.png` files; each other route requested only its own document.

The ordered main-document headers were identical except for `date`:

```text
cache-control: no-store
connection: keep-alive
content-type: text/html; charset=utf-8
date: <recorded below>
keep-alive: timeout=5
transfer-encoding: chunked
```

| Route | Status | Date header | 390px layout | Cookies before/after | Request count |
|---|---:|---|---:|---|---:|
| `/` | 200 | Tue, 21 Jul 2026 22:18:35 GMT | 390 / 390 | `[]` / `[]` | 7 |
| `/why-riot/` | 200 | Tue, 21 Jul 2026 22:18:37 GMT | 390 / 390 | `[]` / `[]` | 1 |
| `/guide/` | 200 | Tue, 21 Jul 2026 22:18:38 GMT | 390 / 390 | `[]` / `[]` | 1 |
| `/about/` | 200 | Tue, 21 Jul 2026 22:18:39 GMT | 390 / 390 | `[]` / `[]` | 1 |
| `/privacy/` | 200 | Tue, 21 Jul 2026 22:18:40 GMT | 390 / 390 | `[]` / `[]` | 1 |
| `/open-source/` | 200 | Tue, 21 Jul 2026 22:18:41 GMT | 390 / 390 | `[]` / `[]` | 1 |
| `/community/` | 200 | Tue, 21 Jul 2026 22:18:42 GMT | 390 / 390 | `[]` / `[]` | 1 |
| `/releases/` | 200 | Tue, 21 Jul 2026 22:18:43 GMT | 390 / 390 | `[]` / `[]` | 1 |
| `/protocols/` | 200 | Tue, 21 Jul 2026 22:18:45 GMT | 390 / 390 | `[]` / `[]` | 1 |

`/resilience/` returned a direct `404` with no `Location` header. The contract also verified absent
source/public resilience directories, static cookie/storage/beacon/network predicates, safe resource
schemes, valid `srcset` descriptors, decoded SVG-favicon safety, and no off-origin subresources.

The final evidence above was refreshed after the prerequisite clarification and addition of the
390px layout assertion. For durable comparison, the complete preceding browser-boundary pass is
preserved verbatim below; it exercised the same route, cookie, header, and network predicates and has SHA-256
`634f4d5d5e651abe4d7cf8c6546781a8e5c03986e1e73d9984dfe7e0865da0a7`:

```json
{
  "origin": "http://127.0.0.1:51722",
  "routes": [
    {
      "route": "/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/",
        "http://127.0.0.1:51722/assets/screenshots/app-checklist.png",
        "http://127.0.0.1:51722/assets/screenshots/app-decisions.png",
        "http://127.0.0.1:51722/assets/screenshots/app-dispatches.png",
        "http://127.0.0.1:51722/assets/screenshots/app-events.png",
        "http://127.0.0.1:51722/assets/screenshots/app-home.png",
        "http://127.0.0.1:51722/assets/screenshots/app-photos.png"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:25 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        },
        {
          "url": "http://127.0.0.1:51722/assets/screenshots/app-checklist.png",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "image/png"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:26 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        },
        {
          "url": "http://127.0.0.1:51722/assets/screenshots/app-decisions.png",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "image/png"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:26 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        },
        {
          "url": "http://127.0.0.1:51722/assets/screenshots/app-dispatches.png",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "image/png"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:26 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        },
        {
          "url": "http://127.0.0.1:51722/assets/screenshots/app-events.png",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "image/png"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:26 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        },
        {
          "url": "http://127.0.0.1:51722/assets/screenshots/app-home.png",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "image/png"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:26 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        },
        {
          "url": "http://127.0.0.1:51722/assets/screenshots/app-photos.png",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "image/png"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:26 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": [
        "http://127.0.0.1:51722/assets/screenshots/app-checklist.png",
        "http://127.0.0.1:51722/assets/screenshots/app-decisions.png",
        "http://127.0.0.1:51722/assets/screenshots/app-dispatches.png",
        "http://127.0.0.1:51722/assets/screenshots/app-events.png",
        "http://127.0.0.1:51722/assets/screenshots/app-home.png",
        "http://127.0.0.1:51722/assets/screenshots/app-photos.png"
      ]
    },
    {
      "route": "/why-riot/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/why-riot/"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/why-riot/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:27 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": []
    },
    {
      "route": "/guide/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/guide/"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/guide/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:28 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": []
    },
    {
      "route": "/about/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/about/"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/about/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:29 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": []
    },
    {
      "route": "/privacy/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/privacy/"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/privacy/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:30 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": []
    },
    {
      "route": "/open-source/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/open-source/"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/open-source/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:31 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": []
    },
    {
      "route": "/community/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/community/"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/community/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:32 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": []
    },
    {
      "route": "/releases/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/releases/"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/releases/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:33 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": []
    },
    {
      "route": "/protocols/",
      "cookiesBefore": [],
      "cookiesAfter": [],
      "documentCookie": "",
      "requests": [
        "http://127.0.0.1:51722/protocols/"
      ],
      "responses": [
        {
          "url": "http://127.0.0.1:51722/protocols/",
          "status": 200,
          "headers": [
            [
              "cache-control",
              "no-store"
            ],
            [
              "connection",
              "keep-alive"
            ],
            [
              "content-type",
              "text/html; charset=utf-8"
            ],
            [
              "date",
              "Tue, 21 Jul 2026 21:56:34 GMT"
            ],
            [
              "keep-alive",
              "timeout=5"
            ],
            [
              "transfer-encoding",
              "chunked"
            ]
          ]
        }
      ],
      "resources": []
    }
  ],
  "resilience": {
    "status": 404,
    "location": null
  }
}
```

## Isolated first-read reviews

Each session ran from a fresh mode-0700 directory containing only one byte-for-byte copy of the
rendered Why Riot HTML and its role prompt. The processes used `codex exec --skip-git-repo-check
--sandbox read-only --json`; sessions were not resumed or shown another reader's answer.

### Community participant/organizer

Session: `019f868f-1465-7533-a6c8-140708c01efa`

Prompt SHA-256: `03aee26ad7277ab70c6a73f3e108cef90df10331d4fd4cedd798f25b372e7be8`

Exact prompt:

```text
Read only the attached rendered Why Riot HTML. Do not inspect the repository, design documents,
prior reviews, or external sources. You are an independent first-time reader in the role: Community participant/organizer.
Answer each assigned question in plain language using only what the page communicates. Return JSON
with role, answers (keyed Q1–Q5 as applicable), and primary_impression. Do not score yourself.

The attached rendered Why Riot HTML is the single local file why-riot.html.

Q1. What kind of ordinary community life is Riot trying to support?
Q2. What four kinds of work does Riot make easier?
Q3. Why might the same tools matter when conditions become difficult?
Q4. What is not currently guaranteed or private?
```

Returned JSON, verbatim:

```json
{
  "role": "Community participant/organizer",
  "answers": {
    "Q1": "Riot supports ordinary shared life: meals, meetings, gardens, block parties, repairs, childcare, local publishing, decisions, care, celebrations, and shared work.",
    "Q2": "Publishing, meeting, coordinating, and carrying collective memory or community data.",
    "Q3": "Regular use builds shared habits: people learn where knowledge lives, how decisions are recorded, and how to work together. Those habits are already available when conditions become difficult.",
    "Q4": "Riot does not currently guarantee anonymity, safe devices, hidden metadata, delivery, persistence, or an available compatible connection. Public content is readable and copyable, and private encrypted groups are not shipped."
  },
  "primary_impression": "Riot is a prototype for community-owned tools that help people build, record, and continue shared life together, while openly stating its limits."
}
```

Orchestrator score: **4/4 PASS**.

- Q1: ordinary community is the subject; names meals, meetings, gardens, repairs, childcare, and more.
- Q2: Publish, Meet, Coordinate, Carry all present.
- Q3: names already-practiced habits and does not guarantee operation.
- Q4: names readable/copyable public content, no encrypted groups, no anonymity, metadata/safety,
  delivery, and persistence guarantees.
- Primary impression is community tools, not privacy, disaster survival, or protocol.

### Potential partner/institution

Session: `019f868f-14de-7dc3-b256-10ca17d55c0d`

Prompt SHA-256: `8b3fe304e57a07296c8c2ea8475148f88a5b8abd5f69ca67019967d94d17d1ce`

Exact prompt:

```text
Read only the attached rendered Why Riot HTML. Do not inspect the repository, design documents,
prior reviews, or external sources. You are an independent first-time reader in the role: Potential partner/institution.
Answer each assigned question in plain language using only what the page communicates. Return JSON
with role, answers (keyed Q1–Q5 as applicable), and primary_impression. Do not score yourself.

The attached rendered Why Riot HTML is the single local file why-riot.html.

Q1. What kind of ordinary community life is Riot trying to support?
Q2. What four kinds of work does Riot make easier?
Q3. Why might the same tools matter when conditions become difficult?
Q4. What is not currently guaranteed or private?
Q5. What can a community possess rather than merely access, and why does that matter?
```

Returned JSON, verbatim:

```json
{
  "role": "Potential partner/institution",
  "answers": {
    "Q1": "Riot supports ordinary community life such as meals, meetings, gardens, block parties, repair work, local publishing, childcare, care, decisions, celebrations, and shared projects.",
    "Q2": "Riot makes it easier to publish, meet, coordinate, and carry community knowledge and memory.",
    "Q3": "Shared practice and familiarity already exist when conditions become difficult, helping people continue building and coordinating together.",
    "Q4": "Riot does not currently guarantee anonymity, safety, hidden metadata, delivery, persistence, or an available transport path. Public content is readable and copyable, and private encrypted groups are not shipped.",
    "Q5": "A community can possess its own data, rather than only accessing a provider's service. This reduces dependence on one provider and helps the community remain connected if it leaves that provider."
  },
  "primary_impression": "Riot is a transparent prototype for community-owned tools that support everyday collective life while clearly acknowledging its limits."
}
```

Orchestrator score: **5/5 PASS**. Shared Q1–Q4 satisfy the same four elements above. Q5 identifies
participant-held data and reduced—not total—dependence on one provider. Primary impression is
everyday collective life, not privacy, disaster survival, or protocol.

### Builder/technical reader

Session: `019f868f-1476-76b0-bf4b-1e0d8ee06697`

Prompt SHA-256: `79cf37366218c2c7500d4dde5121cbce8091b4bee7cbdcc39bfa615466602e12`

Exact prompt:

```text
Read only the attached rendered Why Riot HTML. Do not inspect the repository, design documents,
prior reviews, or external sources. You are an independent first-time reader in the role: Builder/technical reader.
Answer each assigned question in plain language using only what the page communicates. Return JSON
with role, answers (keyed Q1–Q5 as applicable), and primary_impression. Do not score yourself.

The attached rendered Why Riot HTML is the single local file why-riot.html.

Q1. What kind of ordinary community life is Riot trying to support?
Q2. What four kinds of work does Riot make easier?
Q3. Why might the same tools matter when conditions become difficult?
Q4. What is not currently guaranteed or private?
Q5. What bounded mechanism and current-status distinctions make the claim plausible?
```

Returned JSON, verbatim:

```json
{
  "role": "Builder/technical reader",
  "answers": {
    "Q1": "Riot supports ordinary shared life: meals, meetings, stories, decisions, celebrations, care, repair, local publishing, and collective work.",
    "Q2": "Publish, meet, coordinate, and carry.",
    "Q3": "Using the tools in everyday life builds shared habits around storing knowledge, recording decisions, and correcting one another. Those habits remain useful when conditions become difficult.",
    "Q4": "Riot does not currently guarantee anonymity, safe devices, hidden metadata, delivery, persistence, or an available compatible path. Public content is readable and copyable, and private encrypted groups are not shipped.",
    "Q5": "The claim is bounded by local data ownership, signed records that verify control of a key rather than truth, and multiple transport paths such as bundle files, QR references, nearby exchange, gateways, and anchors. Their statuses are distinguished as available in the prototype, tested locally, in development, or direction rather than shipped."
  },
  "primary_impression": "Riot presents itself as practical community infrastructure that reduces dependence on one provider while clearly acknowledging its prototype status and security limits."
}
```

Orchestrator score: **5/5 PASS**. Shared Q1–Q4 satisfy all four elements. Q5 identifies participant-
held/local data, bounded signature meaning, multiple paths, and all four relevant status distinctions.
Primary impression is practical community infrastructure, not privacy, disaster survival, or protocol.

## Isolated semantic claim audit

The fresh audit directory contained only the following ordered artifacts and the audit prompt.

```text
05f6dd79d36a92ff6cf7fc6d719d89a87793e6f235dfe2efab0e35f33e7427a0  marketing/public/index.html
b209f0073f39d4feee8ebb33fc681d2c002748f607004eaac371c2b75bf77ce9  marketing/public/why-riot/index.html
2585319772c6247d21c82bf15ac7bb119bfef7ef157585e14196069033cc0932  marketing/public/guide/index.html
0d0fce89d7ca62d1a450de21f212afd34ac49c1c5e40321852470774c3107d54  marketing/public/about/index.html
ee6594d510d91f0e50fd57974661e5986824b7d91e7a5b866991670512e4bca4  marketing/public/privacy/index.html
2469a37de9060c132086c76b17feaf4bdfbe6e6ab61f2498b4bfddeea0564964  marketing/public/open-source/index.html
34e5f8eae3e10c0f19b2427e86e8f881e5e71f37f57679f2139a133a37fc272d  marketing/public/community/index.html
33113bd9ffb19b4f4ee2215fcfc759e04a08d4e21fb3ffceb4d2658b87a747bb  marketing/public/releases/index.html
d2e7843cf12ba9da3534a6bf782de4d50c5ac2391c86954644fd3587441b00b1  marketing/public/protocols/index.html
f1bac257953347d8903055a4e1252673726a2f24b38c3b2b8cca816aeff7ab83  README.md
a4662882ce7500fba005d284afadef9a4834aaf3d3506e3245dec3351e96e35f  docs/product/product-brief.md
```

Session: `019f86af-8a7d-7140-9336-e977880075ae`

Prompt SHA-256: `0cf03d185fe906bab2bc5c85c8f4b82c37d95947e6f38ff708ce59a1f1ba93db`

Exact prompt:

```text
Review only the attached nine rendered marketing HTML files and two product Markdown documents. Find present-tense or absolute claims
that mean any of: impossible to censor or shut down; always available; guaranteed delivery,
discovery, synchronization, persistence, or recovery; nothing can be lost; anonymous,
confidential, or private-by-default public spaces; audited, field-proven, production-ready, or
operating at scale. A bounded statement naming prerequisites or clearly labeled aspiration is not a
finding. Return JSON with verdict PASS or FAIL and findings containing route, exact excerpt, and
which category it violates.
```

The fresh session completed the audit in one pass. No implementation commentary or prior audit
results were included in its isolated input.

Returned JSON, verbatim:

```json
{"verdict":"PASS","findings":[]}
```

Verdict: **PASS with zero findings**.

## Public-clarity correction — predeployment evidence

This pass removes the homepage comparison badges, the internal research-process scoreboard, and the
generic exposure inventory that framed Riot through fear. It keeps the detailed capability status
section and technical field guide, while stating the current product boundary directly: Newswires
are public publishing and collaboration spaces; private encrypted groups are future work.

### Refreshed visual and browser evidence

The corrected public mirror was served from `http://127.0.0.1:4173`. Playwright 1.61.1 with Chromium
captured and inspected the following artifacts:

| Capture | Viewport/check | SHA-256 |
|---|---|---|
| `/tmp/visual-review/riot-human-capacity/home-desktop.png` | 1456×900 | `9c85b2540b3614fcd662373858630319d4c731a7694bdbf5546b96157a4b5b9d` |
| `/tmp/visual-review/riot-human-capacity/home-mobile.png` | 390×844 | `2783461c2dca678d40088af451c28cbf91ef1a226c2939cb5312c46e1d00e1cc` |
| `/tmp/visual-review/riot-human-capacity/why-riot-desktop.png` | 1456×900 | `9979d814ba6b2906f69a5b4df734f0abf430d18f22dfe0cdb82a48d7386d1b7b` |
| `/tmp/visual-review/riot-human-capacity/why-riot-mobile.png` | 390×844 | `fc4b8f65ab98e8901528f5a41337493a90e04e5ad80b972d7ee2353c03910682` |
| `/tmp/visual-review/riot-human-capacity/privacy-desktop.png` | 1456×900 | `aa1ab2a407b740cbb31cd706768bd83c7b7e298d0671802a55e0d79ee16430c5` |
| `/tmp/visual-review/riot-human-capacity/privacy-mobile.png` | 390×844 | `776692d0a9ccf99e65bf3deeb41c5095e0fe32d0b4c330bd49781acd0f726611` |
| `/tmp/visual-review/riot-human-capacity/why-riot-forced-colors.png` | forced colors, 1456×900 | `afdc3d5a9a59aee39def580ea6f3b821b5aa2172e8dc80579d70cf5a95b38a35` |
| `/tmp/visual-review/riot-human-capacity/privacy-forced-colors.png` | forced colors, 1456×900 | `7206491729986835b0351a9ad1f66a2a47317c5abe06da7a722ff8a32dfa336b` |
| `/tmp/visual-review/riot-human-capacity/why-riot-no-css.png` | no CSS, 390×844 | `cccf6e12617b1fa0e0ed1e8970f6ac83461286072997bc8c084e67eb964f30e7` |

Focused captures verified the changed regions directly: the comparison has no badges
(`1be731f1bc4c09deab369a2eea6cc04d7d5f025c47536458203a304b182a48c3`), the field-history
section flows without the agent-process box
(`a6a08ac52832bd562f6497c6205ce729e49f82d58bb9e56d766fe366437bf738`), the Why Riot boundary is
compact and affirmative (`b96051cb1a28f9d5e6492cadc3b402b3017d0ee3dda0752cef6696495a9cb2fe`),
and the complete Privacy page follows the intended hierarchy
(`0238ee2e2217455ac7aeff4beded4a840a0f4735ac483931f5ed74ed935ffe40`).

All six standard captures had exact `scrollWidth === clientWidth`. Computed contrast evaluation
walked every visible direct-text element and interactive control: Home had 102 unique pairs at each
viewport, Why Riot had 36 desktop/37 mobile, and Privacy had 22 at each viewport. There were zero
WCAG AA failures and zero unresolved backgrounds. The review exposed and fixed a pre-existing
homepage footer defect (pale text and blue links without the intended dark background). Forced-colors
captures remained legible and structured. Without CSS, the Why Riot illustration ended at y=930.56
and the next section began at y=950.56, so it neither overlapped content nor caused horizontal
overflow.

The ordered local HTTP/browser record is
`/tmp/visual-review/riot-human-capacity/clarity-browser-evidence-e83a035.json`, SHA-256
`05b7b71057886dc5ae2456a3ee66eddc1bf2793ff99bd0923ff919aaafb528e2`. Listeners were attached
before navigation; each of the nine routes was fully scrolled and waited for network idle. Every
route returned its document, had empty context cookies before and after, empty `document.cookie`,
`localStorage`, and `sessionStorage`, no `Set-Cookie`, no off-origin requests or performance
resources, and 390/390 layout width. The homepage made seven same-origin requests (document plus six
screenshots); every other route made one. `/__riot_clarity_missing__` returned direct 404 with no
`Location`.

### Refreshed isolated first reads

The exact reviewed Why Riot file was:

```text
0d5bde7fbaacec54cd0ba2f7a60f7172a4326971a246ddca75741aee9bda7a24  marketing/public/why-riot/index.html
```

Each reader ran in a fresh read-only `codex exec` process and received only `why-riot.html` plus its
prompt. The process/session identifiers were `2346` (community), `53302` (partner), and `83656`
(builder); no session was resumed or shown another answer.

#### Community participant/organizer

Prompt SHA-256: `657b65b8da2884115c3cd219406782f22f70d62b81a59afde3817206372206f2`

```text
Read only the attached rendered Why Riot HTML. Do not inspect the repository, design documents,
prior reviews, or external sources. You are an independent first-time reader in the role: Community participant/organizer.
Answer each assigned question in plain language using only what the page communicates. Return JSON
with role, answers (keyed Q1–Q5 as applicable), and primary_impression. Do not score yourself.

Q1. What kind of ordinary community life is Riot trying to support?
Q2. What four kinds of work does Riot make easier?
Q3. Why might the same tools matter when conditions become difficult?
Q4. Which Riot spaces are public today, and what private mode remains future work?
```

Returned JSON, verbatim:

```json
{
  "role": "Community participant/organizer",
  "answers": {
    "Q1": "Riot supports everyday collective life: meals, meetings, stories, decisions, celebrations, care, local projects, and shared work.",
    "Q2": "Publishing, meeting, coordinating, and carrying community knowledge and records.",
    "Q3": "Practicing these tools during ordinary times builds shared habits for recording knowledge, making decisions, and coordinating work. Those habits remain useful when conditions become difficult.",
    "Q4": "Public Newswires are available today for publishing and collaboration. Private encrypted groups remain a future direction; they are not part of today's prototype."
  },
  "primary_impression": "Riot is presented as a set of public, adaptable tools that help communities organize and preserve their shared work without depending entirely on one provider."
}
```

Orchestrator score: **4/4 PASS**. Q1 names people and several ordinary-life examples; Q2 names
Publish, Meet, Coordinate, and Carry; Q3 names already-practiced habits that remain useful without a
guarantee; Q4 identifies current public Newswires and future private encrypted groups. The primary
impression is community tools, not a privacy messenger, disaster product, or protocol project.

#### Potential partner/institution

Prompt SHA-256: `c48ec499023ef94f1ac7f2edb4bf3cf1e2f9a49bdcb834e849e897c3f70a9e19`

```text
Read only the attached rendered Why Riot HTML. Do not inspect the repository, design documents,
prior reviews, or external sources. You are an independent first-time reader in the role: Potential partner/institution.
Answer each assigned question in plain language using only what the page communicates. Return JSON
with role, answers (keyed Q1–Q5 as applicable), and primary_impression. Do not score yourself.

Q1. What kind of ordinary community life is Riot trying to support?
Q2. What four kinds of work does Riot make easier?
Q3. Why might the same tools matter when conditions become difficult?
Q4. Which Riot spaces are public today, and what private mode remains future work?
Q5. What can a community possess rather than merely access, and why does that matter?
```

Returned JSON, verbatim:

```json
{
  "role": "Potential partner/institution",
  "answers": {
    "Q1": "Riot supports everyday community life: meals, meetings, gardens, celebrations, childcare, repairs, local publishing, care, shared decisions, and collective memory.",
    "Q2": "Publish, meet, coordinate, and carry.",
    "Q3": "Regular use builds shared habits and knowledge about where information lives, how decisions are recorded, and how people work together. Those habits remain useful when conditions become difficult.",
    "Q4": "Current public Newswires support publishing and collaboration. Private encrypted groups are a separate future direction, not part of today's prototype.",
    "Q5": "A community can possess its data and installed tools instead of merely accessing a provider's service. This reduces dependence on one provider and helps a community leave a provider without leaving one another."
  },
  "primary_impression": "Riot is a public, prototype toolkit for helping communities publish, meet, coordinate, and preserve their shared work without relying entirely on one provider."
}
```

Orchestrator score: **5/5 PASS**. Q1–Q4 satisfy the shared rubric. Q5 identifies participant-held
data/tools and reduced dependence on one provider without claiming total independence. The primary
impression remains public community work.

#### Builder/technical reader

Prompt SHA-256: `d0b8d10b1dcfb5d5deb7c5d817b593030bcd597a227e8f1397c091ae4a005a2c`

```text
Read only the attached rendered Why Riot HTML. Do not inspect the repository, design documents,
prior reviews, or external sources. You are an independent first-time reader in the role: Builder/technical reader.
Answer each assigned question in plain language using only what the page communicates. Return JSON
with role, answers (keyed Q1–Q5 as applicable), and primary_impression. Do not score yourself.

Q1. What kind of ordinary community life is Riot trying to support?
Q2. What four kinds of work does Riot make easier?
Q3. Why might the same tools matter when conditions become difficult?
Q4. Which Riot spaces are public today, and what private mode remains future work?
Q5. What bounded mechanism and current-status distinctions make the claim plausible?
```

Returned JSON, verbatim:

```json
{
  "role": "Builder/technical reader",
  "answers": {
    "Q1": "Riot supports ordinary community life such as meals, meetings, gardening, block parties, repairs, local publishing, childcare, decisions, care, celebrations, and shared work.",
    "Q2": "Riot makes it easier to publish, meet, coordinate, and carry community knowledge and memory.",
    "Q3": "Using these tools in everyday life builds shared habits for recording knowledge, making decisions, correcting one another, and working together. Those habits remain useful when conditions become difficult.",
    "Q4": "Public Newswires are available today for publishing and collaboration. Private encrypted groups are a future direction, not part of the current prototype.",
    "Q5": "The claim is bounded by participant-held data, signed records, local use on functioning devices, and several transport paths: bundle files, QR references, nearby exchange, public gateways, and anchors. The page distinguishes what is available in the prototype, tested locally, in development, or only a future direction. It therefore promises reduced dependence on one provider—not total independence or a complete reachable copy."
  },
  "primary_impression": "Riot presents itself as a practical prototype for helping communities publish, decide, coordinate, and preserve shared work, while being explicit about current limits and unfinished capabilities."
}
```

Orchestrator score: **5/5 PASS**. Q1–Q4 satisfy the shared rubric. Q5 identifies participant-held
replicas, signed records, multiple paths, and all four required current-status distinctions. The
primary impression remains practical community work.

### Refreshed isolated semantic claim audit

The audit process/session identifier was `79059`. It received only the exact prompt below and these
ordered eleven files:

```text
d1c99bcdc37e4adc29e5acfa5c9571345aa8927351848c63f4e1a5145d510d8b  marketing/public/index.html
0d5bde7fbaacec54cd0ba2f7a60f7172a4326971a246ddca75741aee9bda7a24  marketing/public/why-riot/index.html
2585319772c6247d21c82bf15ac7bb119bfef7ef157585e14196069033cc0932  marketing/public/guide/index.html
0d0fce89d7ca62d1a450de21f212afd34ac49c1c5e40321852470774c3107d54  marketing/public/about/index.html
672ad2c1b5848f11a88ace7a9cba54b20999d858b8d174d838d7a96ec074eae1  marketing/public/privacy/index.html
2469a37de9060c132086c76b17feaf4bdfbe6e6ab61f2498b4bfddeea0564964  marketing/public/open-source/index.html
34e5f8eae3e10c0f19b2427e86e8f881e5e71f37f57679f2139a133a37fc272d  marketing/public/community/index.html
33113bd9ffb19b4f4ee2215fcfc759e04a08d4e21fb3ffceb4d2658b87a747bb  marketing/public/releases/index.html
d2e7843cf12ba9da3534a6bf782de4d50c5ac2391c86954644fd3587441b00b1  marketing/public/protocols/index.html
f1bac257953347d8903055a4e1252673726a2f24b38c3b2b8cca816aeff7ab83  README.md
a4662882ce7500fba005d284afadef9a4834aaf3d3506e3245dec3351e96e35f  docs/product/product-brief.md
```

Prompt SHA-256: `0cf03d185fe906bab2bc5c85c8f4b82c37d95947e6f38ff708ce59a1f1ba93db`

```text
Review only the attached nine rendered marketing HTML files and two product Markdown documents. Find present-tense or absolute claims
that mean any of: impossible to censor or shut down; always available; guaranteed delivery,
discovery, synchronization, persistence, or recovery; nothing can be lost; anonymous,
confidential, or private-by-default public spaces; audited, field-proven, production-ready, or
operating at scale. A bounded statement naming prerequisites or clearly labeled aspiration is not a
finding. Return JSON with verdict PASS or FAIL and findings containing route, exact excerpt, and
which category it violates.
```

Returned JSON, verbatim:

```json
{"verdict":"PASS","findings":[]}
```

Verdict: **PASS with zero findings**.

### Public-clarity production deployment

The reviewed HTML artifact at commit `d6c1b8ed49d9fab5e7bb75b0dee23462f4e8e1ad` was deployed with
the static CSP policy committed at `cceb453` and verified by the production checker at `a4d6b60`.
The final Cloudflare Workers version is `e2e4e09e-401a-4240-8495-e749473325f6`.

Deployment command, run from the detached reviewed-artifact worktree's `marketing/` directory:

```sh
CI=1 WRANGLER_SEND_METRICS=false npx wrangler deploy
```

Post-deploy command:

```sh
npm run verify:marketing:live
```

The complete JSON is
`/tmp/riot-clarity-live-verification-final.json`, SHA-256
`936f4b23296d4e743fd8cb0986b6c702c0d6e947fae75625e994185de3eecd3a`. The command returned
`PASS` for both `https://riot-protest-net-marketing.protestnet.workers.dev` and
`https://riot.protest.net`.

At both origins every route returned direct HTTP 200 with no `Location` or `Set-Cookie`, and raw
response bytes exactly matched the reviewed local file:

| Route | Expected/live SHA-256 at both origins |
|---|---|
| `/` | `d1c99bcdc37e4adc29e5acfa5c9571345aa8927351848c63f4e1a5145d510d8b` |
| `/why-riot/` | `0d5bde7fbaacec54cd0ba2f7a60f7172a4326971a246ddca75741aee9bda7a24` |
| `/guide/` | `2585319772c6247d21c82bf15ac7bb119bfef7ef157585e14196069033cc0932` |
| `/about/` | `0d0fce89d7ca62d1a450de21f212afd34ac49c1c5e40321852470774c3107d54` |
| `/privacy/` | `672ad2c1b5848f11a88ace7a9cba54b20999d858b8d174d838d7a96ec074eae1` |
| `/open-source/` | `2469a37de9060c132086c76b17feaf4bdfbe6e6ab61f2498b4bfddeea0564964` |
| `/community/` | `34e5f8eae3e10c0f19b2427e86e8f881e5e71f37f57679f2139a133a37fc272d` |
| `/releases/` | `33113bd9ffb19b4f4ee2215fcfc759e04a08d4e21fb3ffceb4d2658b87a747bb` |
| `/protocols/` | `d2e7843cf12ba9da3534a6bf782de4d50c5ac2391c86954644fd3587441b00b1` |

At both origins the unknown route returned direct HTTP 404 with no `Location` or `Set-Cookie`.
Browser contexts began and ended with empty cookie jars; `document.cookie`, `localStorage`, and
`sessionStorage` remained empty on all eighteen visits. Every successful browser response was
same-origin.

The first production pass exposed Cloudflare Web Analytics injection on the custom domain. The
static `_headers` policy now sends `script-src 'none'` (plus same-origin style, image, font, and
connection boundaries) at both origins. Chromium still exposes Cloudflare's attempted
`static.cloudflareinsights.com/beacon.min.js` insertion as a request object, but all nine attempts
fail locally with `errorText: csp`; no third-party response occurs and no third-party resource loads.
The Workers origin makes no such attempt. The verifier has a regression test that rejects every
off-origin attempt unless CSP blocks it before any response.
