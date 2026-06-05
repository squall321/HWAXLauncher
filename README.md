# HWAX Agent (HWAXLauncher)

> 사내 CAE/전처리/플러그인 모듈을 **트레이 상주 에이전트**를 통해 받고, 검증하고,
> 실행하는 Windows 데스크탑 런처. [HEAXHub](https://github.com/squall321) 자매 프로젝트.

**한 줄:** 트레이에 살고, 매니페스트를 받아오고, SHA-256 으로 검증하고, atomic 하게 swap 하고,
사용자가 부르면 실행한다. 그 외 모든 표면(자유 URL 입력, 임의 exe 실행, 관리자 권한,
레지스트리 수정)은 **고의로 좁힌다.**

---

## 확정 스택 — 이것만 사용

**Tauri 2 (Rust core) + React 18 + TypeScript + Vite + Tailwind**

다음 스택은 명시적으로 **탈락**, 절대 사용 금지: WinUI 3 / WPF / .NET / C# / XAML /
MAUI / Avalonia / Electron / Flutter.

단일 진실(Source of Truth)은 HEAXHub 레포의
[`docs/hwax-launcher-plan-v2.md`](https://github.com/squall321) (구현자 헌법) 와
[`contracts/hwax-agent/`](contracts/hwax-agent/) (JSON Schema · OpenAPI · 디자인 토큰)
입니다. v2 에 없거나 충돌하는 결정은 이전 안의 잔재 — 무시합니다.

## 저장소 레이아웃

```
HWAXLauncher/
├─ apps/agent/                 Tauri 2 앱
│  ├─ src/                     React 18 + TS UI (트레이 패널, 설정, 모듈 목록)
│  │  ├─ ipc/                  invoke 래퍼 + Rust 커맨드 타입 (단일 IPC 표면)
│  │  ├─ panels/  components/  hooks/
│  └─ src-tauri/               Tauri 셸 (얇은 커맨드) — hwax-core 를 호출
│     ├─ src/{commands,tray,auth,sync,telemetry,download}/
│     ├─ capabilities/         최소 권한 capability (shell.open 금지 등)
│     └─ tauri.conf.json       CSP, updater(Ed25519), allowlist
├─ crates/hwax-core/           ⭐ 순수 Rust 로직 (Tauri/WebView2 비의존)
│  └─ src/                     installer·verify·swap·rollback·GC·state machine·
│                              manifest/install-report/audit 모델 + 빌더
├─ packages/
│  ├─ design-tokens/           contracts/tokens.css → TS/Tailwind 토큰
│  └─ schemas/                 contract JSON Schema → TS 타입 + ajv 검증
├─ contracts/hwax-agent/       벤더링된 계약 (v0.2.0) — 단일 진실 미러
├─ scripts/                    sign.ps1 · publish.ps1 · fetch-schemas.mjs
├─ docs/                       ADR · ARCHITECTURE · RUNBOOK · EDR 화이트리스트
└─ .github/workflows/          build-and-sign.yml · release.yml
```

> **왜 `crates/hwax-core` 를 분리했는가:** 다운로드/검증/swap/롤백/상태기계 같은
> 핵심 로직을 Tauri 런타임·WebView2 없이 `cargo test -p hwax-core` 로 **헤드리스
> 검증**하기 위함. v2 §22 의 `src-tauri/{installer,store,...}` 구조는 유지하되,
> 순수 로직만 크레이트로 끌어내 src-tauri 의 커맨드는 얇은 어댑터가 된다.
> (v2 와 충돌하지 않는 구현 세부 — Tauri/Rust 범위 내.)

## 핵심 보안 자세 (v2 §15 · §17)

- 다운로드 URL은 `config.allowed_origins` **정확 매칭**만 허용 (자유 URL 입력 금지).
- 모든 패키지는 manifest `sha256` 강제 검증 — 불일치 시 즉시 `.partial` 삭제.
- zip 압축 해제는 **zip-slip 방어** (각 entry 정규화 후 staging 하위 확인).
- staging → final, current.json 모두 **same-volume rename = atomic swap**.
- 실행은 `manifest.entry.executable` **화이트리스트** 외 절대 금지, 사용자 입력 인자 없음.
- device JWT / refresh token 은 **Windows Credential Manager (keyring)** — 파일 평문 금지.
- 기본 `asInvoker` (관리자 권한 자동 요청 없음), 쓰기는 `%LocalAppData%\HWAXAgent\` 하위만.

## 개발 빠른 시작 (Windows PC)

```powershell
# 사전: Rust >= 1.77, Node 20+, pnpm, WebView2 Runtime
corepack enable
pnpm install
pnpm fetch-schemas              # contracts 동기화 (scripts/fetch-schemas.mjs)

# 순수 로직 헤드리스 테스트 (Tauri 불필요)
cargo test -p hwax-core

# 앱 개발 실행 (트레이 상주)
pnpm tauri dev
```

## 라이선스

사내 비공개(Proprietary). 코드 사인 인증서·서명 키·빌드 산출물은 어떤 경우에도
이 레포에 두지 않는다 (`.gitignore` 강제). 자세한 협업 규약은
HEAXHub `docs/hwax-agent-pr-protocol.md` 참고.
