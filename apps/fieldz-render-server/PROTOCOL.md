# fieldz-render-server wire protocol, v1

The server draws a real FieldCraft window without a window and streams it to one host process
over loopback TCP. Input and the model's engine calls travel back on the same connection, into
the same `Session` the picture is drawn from. The design is `docs/fieldz-render-server.md`; this
file is the normative wire.

- **Transport:** TCP on `127.0.0.1`. The server listens; `--port 0` (the default) takes an
  ephemeral port, printed on stdout as one line, `ready {"v":1,"port":N,"pid":N}` (a generated
  token, when there is one, is added as `"token"`). One connection; a second is never accepted.
- **Framing:** `u32` big-endian header length ≤ 65 536 | JSON header | payload. The header is a
  JSON object whose `"len"` field is the payload's byte length (the server writes it; a reader
  defaults it to 0). Payload ≤ 128 MiB. Inbound messages are header-only in v1.
- **Order:** frames and replies are written through one lock, so they never interleave bytes.
- **Authentication:** the first message must be `hello` with the token from
  `FIELDCRAFT_RENDER_TOKEN` / `--token-file` (or the value printed in `ready` when the server
  generated one). The comparison is constant-time over the bytes. A wrong version or token is
  answered with `error` and the connection is closed. The server draws and sends nothing before
  a valid `hello`.
- **Versions:** `v` is 1. A later protocol is refused rather than guessed.

## 1. Host → server

All messages have `"t"`, and (written by the framing layer) `"len": 0`.

### `hello` (must be first; within 10 s)

| field | type | meaning |
|---|---|---|
| `v` | number | protocol version, `1`. |
| `token` | string | the session token. |

The server answers `ready`.

### `input`

| field | type | meaning |
|---|---|---|
| `events` | array | one pass's events, applied in order. Each is an object with `"k"`. |

Events (`"k"`):

| `k` | fields | egui result (through `fieldcraft_ui_egui::control`) |
|---|---|---|
| `move` | `x`, `y` (CSS points) | `ui.move` → one `Event::PointerMoved(pos)` |
| `button` | `x`, `y`, `button` (`"left"`/`"right"`/`"middle"`), `down`, `double`, `modifiers` | `ui.click` → `PointerMoved` + `PointerButton{pressed:true}` + `PointerButton{pressed:false}`; `double:true` repeats the pair. **A `button` is a whole click**: with `down:false` it is accepted and ignored, so a host sends one `button` per click and uses `drag` for drags. |
| `wheel` | `x`, `y`, `dx`, `dy`, `unit`, `modifiers` | `ui.scroll` → `PointerMoved` + `Event::MouseWheel { unit: Line, delta: (0, dy), phase: Move, source: Wheel }`. `dy` is egui's delta in lines (positive = down/in); `dx` and `unit` are accepted and dropped in v1 (`ui.scroll` is vertical). |
| `key` | `key`, `physical`, `down`, `modifiers` | `ui.key` → `Event::Key{pressed:true}` + `Event::Key{pressed:false}` with the same modifiers. `key` is an `egui::Key` name (`"A"`, `"ArrowDown"`, `"Escape"`, `"Enter"`, `"Delete"`, `"Space"`, `"Tab"`, …). **A whole tap**: `down:false` is ignored. `physical` is accepted and unused in v1 (`ui.key` sends no physical key). |
| `text` | `text` | `ui.text` → `Event::Text(text)` |
| `drag` | `from:[x,y]`, `to:[x,y]`, `steps`, `button`, `modifiers` | `ui.drag` → `PointerMoved(from)`, press, `steps` (1..=200, default 10) interpolated `PointerMoved`s, release at `to` |
| `drop` | `paths` (1..=16 strings, ≤ 4096 bytes each) | `ui.drop` → queued in `ModelApp::synthetic_drops`; the next pass opens them as a window-system drop does |
| `focus` | `gained` (bool) | not a control call: sets `RawInput.focused` for the next pass |

`modifiers` is `{"alt":bool,"ctrl":bool,"shift":bool,"meta":bool}`, all defaulting to false.
The server turns it into egui's `Modifiers` per its own OS: in `ui.key`, `command` is `meta` on
macOS and `ctrl` elsewhere, `ctrl` is `ctrl`. For pointer events `ui.click`/`ui.drag` carry
`shift`, `ctrl` and `alt`; `meta` is not carried by those methods in v1, and `alt` is not
carried by `ui.key`. These are the control methods' existing shapes, not newly invented rules.

The app's `raw_input_hook` then feeds egui at most one pointer event (or one key sequence) per
pass, adding `ModifiersChanged` as egui expects; a pointer press's modifiers hold until its
release.

### `resize`

| field | type | meaning |
|---|---|---|
| `w`, `h` | number | the canvas in CSS points (= points, zoom 1). |
| `dpr` | number | pixels per point. |

The server computes device pixels as `round(w * dpr)` and lowers the dpr when the result would
exceed 8192 pixels a side or 4096×4096 pixels in all; the next frame is `full` and its
`dprUsed` reports what it used. A size change also resets the diff: the next frame is `full`.

### `visible`

| field | type | meaning |
|---|---|---|
| `visible` | bool | The pane is shown. While hidden the server runs passes (jobs must answer) but sends no frames; when shown again it sends `full`. |

### `ack`

| field | type | meaning |
|---|---|---|
| `seq` | number | The frame with this `seq` was shown. Returns one credit. |

### `call`

| field | type | meaning |
|---|---|---|
| `id` | number | The caller's id; the reply carries it. |
| `method` | string | a control method (`engine.execute`, `engine.script`, `engine.commands`, `job.status`, `job.wait`, `job.cancel`, `ui.*`, …), or `engine.batch` / `ui.operations` (this server's two). |
| `params` | object | the method's parameters, as `fieldcraft_ui_egui::control` defines them (or as below). |
| `op` | string, optional | the turn/operation this call belongs to (`"turn/step"`); `ui.operations {cancel}` uses it. Default `"call/<id>"`. |

Answers are exactly `control::handle`'s: `{"ok":true,"result":…}` or `{"ok":false,"error":…}`,
wrapped in `reply` (below). A long command (`solve.*`, a mesher, an open) starts the app's
background job and answers when it finishes; with `"wait": false` it answers at once with
`{"job": n}`. A screenshot answers once a frame has been presented.

**`engine.batch`** (new here): `{"steps": [{"command": id, "params": {…}, "wait": bool}, …]}`,
at most 10 000 steps. Steps run in order on the same `Session`; the first failure stops the
batch and the steps already done stay done (no rollback). The reply's `result` is:

```json
{"ok": true,  "completed": 3, "results": [ … ]}
{"ok": false, "completed": 1, "failed": {"command": "mesh.tet.generate", "error": {"ok": false, "error": "…"}}, "results": [ … ]}
```

A step that is a long command suspends the batch until its job answers. `results` holds each
completed step's `result` value.

**`ui.operations`** (new here): `{"op": name?, "cancel": name|"agent"}`.

- with no `cancel`: answers `{"ok":true,"result":{"operations":[{id, op, elapsed_ms, kind}], "batches": n}}`.
- `cancel: "<op>"`: stops that operation's calls and unstarted batch steps; stops the running
  background job only if it belongs to that operation. Answers
  `{"ok":true,"result":{"cancelled": "<op>", "calls": n}}`; each stopped call is itself answered
  `{"ok":false,"error":"cancelled: <op>","cancelled":true}`.
- `cancel: "agent"`: stops the whole turn: every call in flight, every batch, and (through the
  app) the running job. The reply to this same `call` is the stop confirmation:

```json
{"stopped": true,  "at": 1234, "elapsed_ms": 40}
{"stopped": false, "at": null, "elapsed_ms": 5000, "reason": "ui thread busy"}
```

`stopped: true` means every in-flight call answered (`cancelled`) before the 5 s window closed;
`stopped: false` means the window closed first (the loop was inside a synchronous command that
cannot be preempted). The reader thread writes the `stopped:false` answer itself if the loop
cannot, so the confirmation is always within 5 s; in that answer `at` is `null` (the reader has
no access to the server loop's clock) and `reason` is `"ui thread busy"`. Otherwise `at` is the
server loop's millisecond clock when the stop was taken.

**The 120 s step budget.** Each step's start is timed. A background job's step that reaches
120 s is cancelled and its call answered
`{"ok":false,"error":"stopped: 单步预算 120 秒已用完","code":"budget","stopped":true}`; the UI
keeps running (nothing is torn down). A *synchronous* command cannot be interrupted (the loop
is inside it); when it returns past 120 s its own result is answered with
`"over_budget": true` and `"elapsed_ms"` added. In a batch the budget is per step, timed from
that step's start, and the batch's failure names the step.

### `stop`

| field | type | meaning |
|---|---|---|
| `why` | string, optional | why the host is ending the session. |

The server answers `bye` and exits (≤ 5 s). Closing the connection ends the session the same way.

## 2. Server → host

### `ready` (answer to `hello`)

| field | type | meaning |
|---|---|---|
| `v` | number | protocol version. |
| `w`, `h` | number | canvas in CSS points in effect. |
| `dpr` | number | pixels per point actually used. |
| `pid` | number | the server's process id. |

### `frame`

| field | type | meaning |
|---|---|---|
| `seq` | number | from 1, one per sent frame; acknowledged by `ack`. |
| `w`, `h` | number | the frame in device pixels. |
| `dpr` | number | the dpr the host asked for. |
| `dprUsed` | number | the dpr used (≤ `dpr` when the frame would be too large). |
| `full` | bool | the whole frame is in `rects[0]` (a PNG): the host clears and draws it. |
| `rects` | array | the rectangles, in payload order. |

A rectangle:

| field | type | meaning |
|---|---|---|
| `x`, `y`, `w`, `h` | number | device pixels, top-left origin. |
| `codec` | string | `"raw"` (RGBA bytes), `"jpeg"` (one stripe) or `"png"`. |
| `off`, `len` | number | the bytes' offset and length in the payload. |
| `rows` | number | the stripe's height in pixels (equal to `h` for raw and PNG). |

Encoding: a rectangle of at most 65 536 pixels is raw RGBA; a larger one is cut into JPEG
stripes of at most 256 rows (q80), each its own rectangle so the host decodes them concurrently;
the whole frame (first frame, resize, visible again) is one PNG. A frame never carries more than
256 rectangles: a change scattered that widely (which would overrun the header limit) is sent as
one whole-frame PNG instead. The host blits each rectangle at `(x, y)`. The payload is the
concatenation of the rectangles' bytes.

### `reply`

| field | type | meaning |
|---|---|---|
| `id` | number | the `call`'s id. |
| `result` | object | the control handler's reply (or the server's, for `engine.batch`, `ui.operations` and the budget). |

### `bye`

| field | type | meaning |
|---|---|---|
| `why` | string | `"stop"` or the reason the host gave. |

The session is over; the process exits.

### `error`

| field | type | meaning |
|---|---|---|
| `error` | string | why the connection is closing (bad token, wrong version, malformed message, `hello` not first). |

### Not in v1

The design also lists `cursor` and `status` (monitoring extras) and an idle refinement pass
(re-sending the viewport at q95 after 200 ms without input). Neither is sent in v1: the pane
chooses its own cursor, and the host can ask for the session's state with a `call`
(`ui.inspect`, `model.info`) when it needs it. The first picture of a resize is still an exact
PNG, so what v1 leaves out is speed and sharpness, never correctness.

## 3. Flow control and lifecycle

- Credits: 2 at the start, one spent per `frame`, one returned per `ack`. The server never drops
  a sent frame; with no credit it stops sending and accumulates the changed region, so the next
  frame's rectangles are still exact.
- Passes run on demand: input, `resize`, `visible`, `ack`, a `call`, the app's repaint requests
  and its timers. At most `--fps-cap` (default 60) passes a second. Nothing is encoded when the
  diff is empty, the pane is hidden, or no credit is left.
- Lifecycle: host spawns the server → reads `ready` → `hello` ≤ 10 s → `ready` → frames and
  calls. `stop` → `bye` → exit. A connection close ends the session. A process that exits
  without `bye` is a crash to the host. A synchronous command cannot be preempted, so a `stop`
  that arrives while one runs is answered when it returns; a host that must not wait kills the
  process.

## 4. Example (one click that starts a solve)

```text
→ hello {"v":1,"token":"…"}
← ready {"t":"ready","v":1,"w":1280,"h":800,"dpr":2.0,"pid":4242}
← frame {"t":"frame","seq":1,"w":2560,"h":1600,"dpr":2.0,"dprUsed":2.0,"full":true,
         "rects":[{"x":0,"y":0,"w":2560,"h":1600,"codec":"png","off":0,"len":410233,"rows":1600}],"len":410233}
→ ack   {"t":"ack","seq":1}
→ input {"t":"input","events":[{"k":"move","x":700.0,"y":220.0},
                               {"k":"button","x":700.0,"y":220.0,"button":"left","down":true}]}
→ call  {"t":"call","id":1,"method":"engine.execute","op":"turn/step",
         "params":{"command":"solve.run","params":{"step":"Load"}}}
← frame {"t":"frame","seq":2,"w":2560,"h":1600,"dpr":2.0,"dprUsed":2.0,"full":false,
         "rects":[{"x":1408,"y":0,"w":256,"h":64,"codec":"raw","off":0,"len":65536,"rows":64}],"len":65536}
← reply {"t":"reply","id":1,"result":{"ok":true,"result":{"step":"Load","frames":1}}}
→ stop  {"t":"stop","why":"session ended"}
← bye   {"t":"bye","why":"session ended"}
```