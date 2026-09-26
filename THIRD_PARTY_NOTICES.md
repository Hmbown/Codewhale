# Third-party notices

Source vendored or ported into this repository, beyond the crates resolved by
Cargo (whose licences are enforced by `deny.toml`).

## pi (`pi-mono`) — MIT

`crates/config/src/device_code.rs` is a Rust port of pi's OAuth device-code
polling loop and verification-URI check:

- `packages/ai/src/auth/oauth/device-code.ts` (`pollOAuthDeviceCodeFlow`,
  the RFC 8628 polling behaviours)
- `packages/ai/src/auth/oauth/xai.ts` (`validateVerificationUri`)

Upstream: <https://github.com/badlogic/pi-mono>

```
MIT License

Copyright (c) 2025 Mario Zechner

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## OpenAI Codex (`codex-rs`) — Apache-2.0

`crates/tui/src/tui/frame_rate_limiter.rs` is adapted from Codex's TUI draw-rate
limiter, `codex-rs/tui/src/tui/frame_rate_limiter.rs`. It was simplified for a
poll-based render loop; the file header records the change.

Upstream: <https://github.com/openai/codex>

```
Copyright 2025 OpenAI

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

The full Apache License 2.0 text ships in this repository at
`patches/unicode-width-0.2.2/LICENSE-APACHE`.
