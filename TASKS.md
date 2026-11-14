# TASKS — 작업 현황과 남은 일

Dynalist를 클린룸으로 재구현하는 셀프호스팅 아웃라이너. 이 문서만 읽고 이어서 작업할 수 있게 정리했다.
(최종 갱신: 2026-11-14)

- 레포: https://github.com/jadewisemann/mynalist (public), 로컬 `/home/ubuntu/code/outliner`, 브랜치 `main`
- 구조:
  - `packages/core`: 트리/op 엔진, undo, sync, 에디터 명령, 인라인 문법, 날짜, 검색, 붙여넣기/내보내기. 런타임 의존성 없음.
  - `apps/server`: Node 24, node:sqlite, ws.
  - `apps/web`: Vite, vanilla TS.
  - `deploy/`: systemd 파일.
  - `qa/`: Dynalist 비교 QA.
- 스펙: `docs/01-data-model-sync.md`, `docs/02-editor-keyboard.md`, `docs/03-features-ui.md`, `docs/ARCHITECTURE.md`.

## 지켜야 할 원칙

- **클린룸:**
  - Dynalist 코드, 폰트, 에셋, 데모 문서 내용을 가져오지 않는다.
  - 라이브 데모(`https://dynalist.io/demo`)를 블랙박스로 관찰하는 것은 괜찮다.
  - 폰트는 Fira Sans/Mono를 쓴다 (OFL, `@fontsource`).
- **비밀값은 git에 넣지 않는다:**
  - 레포는 public이다.
  - 토큰과 비밀번호는 `~/.config/outliner/env`(chmod 600)에만 있다.
  - 테스트용 비밀값은 실행할 때마다 생성한다.
- **외부 공개 작업은 사용자가 직접 한다:** Tailscale Funnel 같은 외부 공개 작업은 자동 실행이 막혀 있다.
- **QA 판정 기준:**
  - 동작이 Dynalist와 다르면 Dynalist를 따른다.
  - 단, Dynalist의 버그는 따라 하지 않는다. 대신 `qa/lib/compare.mjs`의 KNOWN 목록에 등록한다.

## 지금까지 한 일

### 앱 (커밋 순)

1. **core:**
   - 트리/op 엔진(`create/set/move/collapse`), 문서 세션, undo, 동기화 rebase.
   - 에디터 명령, 가시성 모델.
2. **sync 서버:**
   - node:sqlite op 로그와 스냅샷, HTTP/WS API, Dockerfile.
   - 삭제 노드는 분리된 지 오래되면 purge.
   - 로그인 실패가 반복되면 429로 막는다.
3. **웹 앱:**
   - 에디터, 동기화 클라이언트, 워크스페이스, 사이드바, 검색, 찾기, 내보내기.
4. **core 텍스트 모듈:** 인라인 파서/렌더러, 날짜, 검색 쿼리, 붙여넣기, export.
5. **에디터:**
   - 성능: 불필요한 row 업데이트 생략, content-visibility.
   - bullet 드래그앤드롭, 항목 컨텍스트 메뉴.
6. **Android:** Capacitor 8 셸이었으나 2026-11-13에 제거했다. 모바일과 macOS는 Rust 네이티브 앱으로 새로 만든다.
7. **PWA:** manifest, 아이콘, service worker. systemd 배포 파일.
8. **반복 날짜:**
   - 반복 날짜 체크 처리(new_instance / update).
   - 에디터 내 팝업: 태그 자동완성, `[[` 항목 링크, 날짜 선택기(반복 포함).
9. **배포 편의:**
   - 루트 `npm start`: web 빌드가 없으면 빌드한 뒤 서빙한다.
   - `SERVER_PORT` 지원.
   - `deploy/install.sh`: 127.0.0.1에만 바인딩하고, node 경로를 고정한다.
10. **스펙 갭 일괄 구현 (2026-10-24, "남은 일" 3번의 P1/P2/P3):**
    - **서버:**
      - 버전 기록 API (`/api/doc/<id>/history`, `/at/<v>`).
      - 백업: `/api/backup`, `outliner-backup.timer`. 업로드 파일도 백업한다.
      - 파일 업로드: `/api/upload`, `/files/<key>/…`.
      - 공유 링크: `/s/<token>`. 서버가 렌더링하는 읽기 전용 페이지다.
      - 캘린더 ICS 피드: `/cal/<key>.ics`.
      - 보안 헤더.
    - **core:**
      - `restoreTree` (버전 복원)
      - 정렬 모드 11종
      - 검색/치환, backlinks
      - `is:completed` 필터, unindent in place
      - ICS 생성, 시간대 기록 (`withLocalTz`)
    - **웹, 검색과 탐색:**
      - Flat/Global 검색 결과 화면
      - Search & replace
      - 버전 기록 모달 (diff, 복원)
    - **웹, 입력과 관리:**
      - Inbox: 빠른 입력, 이동
      - 백업 zip
      - Settings (탭, Keymap 편집, 고급 설정)
      - 도움말 사이드 패널
      - 모바일 툴바 사용자화
    - **웹, 보기:**
      - 테마 4종, 글꼴
      - 문서별 보기 옵션 (체크, 노트, Article, 마인드맵, RTL, 잠금)
      - 코드 하이라이트와 Copy 버튼, KaTeX
    - **웹, 항목:**
      - backlinks 섹션과 배지
      - 사이드바: 툴바, 문서 메뉴, 드래그로 문서 간 이동, 보관 문서는 동기화 생략
      - 북마크와 태그 패널 기능
      - 단축키 추가
      - 이미지 미리보기, 튜토리얼, What's new
      - 파일 붙여넣기/드롭 업로드
      - 공유 대화상자, 업로드/공유/캘린더 관리 화면

### 배포 (이 머신, oci-agent)

- **서비스:**
  - systemd user 서비스 `outliner.service`가 `127.0.0.1:8080`에서 실행 중이다.
  - linger가 켜져 있어 로그아웃해도 계속 돈다.
  - 서비스 파일: `~/.config/systemd/user/outliner.service`.
- **데이터 위치:**
  - DB: `~/.local/share/outliner/outliner.db`.
  - 비밀값: `~/.config/outliner/env`.
- **설정 상태:**
  - `OUTLINER_STATIC=apps/web/dist`.
  - 웹을 고친 뒤에는 `npm run build -w @outliner/web && systemctl --user restart outliner`로 다시 빌드하고 재시작한다.
- **호스팅 검토:**
  - botkeep을 검토했고 코드도 맞춰 두었지만, botkeep 쪽 용량이 없어 이 머신에서 Tailscale Funnel로 공개하기로 했다.
  - Funnel은 아직 실행하지 않았다. 아래 남은 일 1번.

### Dynalist 비교 QA (`qa/`, 자세한 건 `qa/PLAN.md`)

- **방식:**
  - 라이브 Dynalist 데모와 우리 빌드를 headless Chromium에서 같은 시나리오로 돌린다.
  - 정규화한 상태(트리, 속성, 캐럿, 블록 선택, 줌, 클립보드)를 나란히 비교한다.
- **묶음별 결과:**

  | 묶음 | 내용 | 결과 |
  |---|---|---|
  | editing | 시나리오 398개 (search 20개, highlight 35개 포함) | 전부 일치 |
  | fuzz | 랜덤 키 400개 × 10스텝, 매 스텝 비교 | 399개 일치, 1개 건너뜀 |
  | render | 인라인 문법 117개 | 117/117 |
  | layout | 측정값 129개 | 129/129 |
  | layout --mobile | 폰 뷰포트 측정값 129개 | 129/129 |
  | menus | 컨텍스트 메뉴 6가지 상태 | 6/6 |
  | mobile | 폰 헤더/툴바/터치 동작 체크 100개 | 100/100 |

  - editing과 fuzz의 "일치"에는 ±1글자 캐럿 허용, 단독 재실행에서 일치한 타이밍 흔들림, 문서화한 Dynalist 쪽 버그가 포함된다.
  - fuzz에서 건너뛴 1개는 undo로 Dynalist 데모 내용이 되살아나는 경우다. 데모 내용은 저장하지 않으므로 판정만 남긴다.
- **녹화와 비교:**
  - Dynalist 결과를 `qa/golden/`에 녹화해 두었다. 시각은 UTC로 고정했다.
  - `npm run qa`는 오프라인 비교이고, 회귀가 있으면 실패한다.
  - GitHub Actions(`.github/workflows/ci.yml`)에서 매 push/PR마다 단위 테스트와 이 비교를 돌린다. 마지막 실행은 통과했다.
- **QA로 찾아서 고친 동작:**
  - **캐럿과 화살표:**
    - ↑/↓는 note를 건너뛰고, 목표 열을 기억하지 않으며, 줄 맨 앞에서는 대상 줄 맨 앞으로 간다.
    - 체크하면 캐럿이 다음 항목으로 간다.
    - 줌해도 캐럿 위치를 유지한다.
  - **Escape:**
    - 편집을 끄고 켜는 토글이다.
    - 블록 선택에서 Escape를 두 번 누르면 첫 항목 맨 앞에서 편집을 이어간다.
    - 제목에서 Shift+↑/↓를 누르면 Dynalist처럼 "숨은 선택" 상태가 된다.
  - **Undo 후 캐럿:**
    - 타이핑을 되돌리면 입력을 시작한 위치로 간다.
    - 삭제를 되돌리면 첫 삭제 직후 위치로 간다.
  - **삭제:**
    - Delete로 합치면 합쳐진 항목이 다음 항목 자리에 남는다. 빈 항목은 그냥 사라진다.
    - 블록 선택에서 Backspace는 이전 항목으로, Delete는 다음 항목으로 캐럿이 간다.
    - 첫 항목에서 Backspace를 누르면 제목으로 합쳐진다.
  - **이동:** Ctrl+↑/↓로 형제 항목 안으로 들어갈 수 있다.
  - **키 처리:** 단축키로 묶인 키는 브라우저로 새지 않는다.
  - **붙여넣기와 복사:** 일반 텍스트 붙여넣기와 복사 형식을 Dynalist에 맞췄다.
  - **렌더링:**
    - 빈 강조는 표시하지 않는다.
    - 링크 텍스트는 그대로 둔다.
    - 날짜 라벨 형식("Tue, January 2nd 2024, 10 am", 상대 시간)을 맞췄다.
  - **레이아웃:** 크기, 줄 높이, 색, 간격을 Dynalist에서 측정한 값으로 맞췄다.
  - **실제 버그 수정:** 자식이 있는 외동 자식으로 Delete를 누르면 하위 트리가 사라지던 문제.
- **이 규칙들의 단위 테스트:** `packages/core/test/parity.test.ts`. 현재 core/server 테스트 192개가 통과한다.
- **2026-10-24에 추가한 비교:**
  - **search 그룹 (editing에 20개):**
    - 검색 쿼리 문법과 항목 표시 여부를 비교한다.
    - 자식이 모두 걸러진 부모는 접힌 모양으로 보인다.
    - Esc를 누르면 캐럿이 원래 자리로 돌아간다.
  - **`menus` 스위트:** 6가지 항목 상태에서 컨텍스트 메뉴의 그룹, 라벨, 단축키 표기를 비교한다. 6/6 일치.
  - **`layout --mobile`:** 폰 뷰포트에서 Dynalist 모바일 레이아웃과 비교한다. 129/129 일치.
  - **highlight 그룹(35개)과 `mobile` 스위트(100개 체크):**
    - 이 두 비교는 하위 에이전트 병렬 작업으로 추가했다.
    - 폰 상단 헤더와 하단 툴바를 Dynalist와 맞췄다: 툴바는 편집 중에만 보이고, 기본 버튼 구성을 맞췄으며, Select multiple 모드를 추가했다.
  - **이 비교로 고친 것:**
    - 메뉴 라벨과 순서를 맞췄다.
    - 단축키 표기를 ↵/⌫로 바꿨다.
    - 모바일에서 들여쓰기는 21px, 체크박스 터치 영역은 28px로 했다.
    - 모바일에서 문서를 열어도 사이드 드로어가 닫히지 않던 버그를 고쳤다.
    - 제목 메뉴의 정렬이 현재 선택 항목에 적용되던 버그를 고쳤다.

## 남은 일

### 1. 공개 배포 마무리 (사용자 작업이 필요)
- [ ] 사용자가 `sudo tailscale funnel --bg 8080`을 실행하고, 링크가 뜨면 HTTPS/Funnel을 승인한다. (자동 실행은 막혀 있다.)
- [ ] 공개 URL(`https://<machine>.<tailnet>.ts.net`)로 접속해 본다:
  - 로그인
  - 설정 → Sync & data에서 같은 URL로 연결
  - 휴대폰에서 PWA 설치
  - 두 기기 간 동기화
- [ ] 휴대폰 실사용 점검: 모바일 툴바, 가상 키보드, 팝업 위치. headless 모바일 뷰포트 비교는 통과했다.
- [x] DB 백업:
  - `outliner-backup.timer`가 매일 04:17에 `VACUUM INTO`로 백업하고 14개를 보관한다.
  - 업로드 파일은 `backups/files/`로 증분 복사한다.
  - 이 머신에 설치했고 활성화되어 있다.
- [x] 서버 보안 점검. 결과는 README "Security notes"에 정리했다:
  - 로그인 제한
  - CORS `*`를 쓰는 근거
  - 본문 크기와 업로드 크기 제한
  - 보안 헤더
  - capability URL 3종 (파일, 공유, 캘린더)

### 2. QA에서 남은 차이와 한계
- [x] **코드 블록:** 문법 강조와 Copy 버튼을 구현했다. 자체 하이라이터를 썼다.
- [x] **HTML 붙여넣기:** `apps/web/e2e/features.mjs`에서 검증한다. 중첩 목록과 굵게가 유지된다.
- [x] **QA 확장:**
  - 검색: editing의 search 그룹
  - 컨텍스트 메뉴와 단축키 표기: `qa/menus.mjs`
  - 모바일 뷰포트: `qa/layout.mjs --mobile`
- [x] **검색 하이라이트 범위:** editing의 `highlight/` 그룹(35개)에서 비교한다.
  - 매칭된 항목만 표시하고, 겹치는 범위는 합친다.
  - 링크 URL에 매칭되면 링크 원문을 보여준다.
  - 이미지 alt도 표시한다.
  - Dynalist 버그 1건(코드 스팬의 백틱)은 KNOWN에 등록했다.
- [x] **모바일 툴바와 터치 동작:** `qa/mobile.mjs`에서 비교한다. 100/100 일치.
  - 헤더 버튼과 툴바 버튼의 종류, 순서, 표시 조건, 크기를 비교한다.
  - 탭, 툴바 명령, 다중 선택, 읽기 모드 동작을 비교한다.
- [ ] **아직 비교하지 않은 것:**
  - 여러 문서, 동기화, 설정, 색 라벨(Ctrl+Shift+L): 데모에 없거나 동작하지 않는다.
  - 모바일 "more" 메뉴 내용, 날짜 선택기·Move to·Get link 대화상자.
- [ ] **녹화 다시 하기:** Dynalist가 바뀌었는지 가끔 `npm run qa:live`로 확인하고, 필요하면 `npm run qa:record`.
- [x] **CI:**
  - 러너를 `ubuntu-24.04`로 고정했다. ubuntu-latest가 26으로 바뀌는 것에 대비한 것이다.
  - `checkout@v6`, `upload-artifact@v7`로 올렸다.
  - push 후 통과하는지 확인할 것.

### 3. 스펙 대비 미구현/부분 구현 기능
P1과 P2는 모두 구현했다. 구현 내용은 위 "지금까지 한 일" 10번에 있다.
- [x] **데스크톱 항목 툴바:**
  - hover하면 행 위에 버튼 줄이 뜬다. 레이아웃은 밀리지 않는다.
  - 설정 "Show item toolbar on hover (desktop)"으로 켠다. 기본은 꺼짐이다.
- [x] **접기/펼치기 애니메이션:**
  - 상태는 즉시 바뀌고, 높이만 180ms 동안 애니메이션한다.
  - `prefers-reduced-motion`과 검색 중에는 끈다.
  - 키를 누르면 애니메이션을 즉시 끝낸다. 그래서 QA용으로 따로 끌 필요가 없다.

**P3 (단일 사용자 셀프호스팅 기준):**
- [x] 파일 업로드:
  - 붙여넣기/드롭으로 업로드한다.
  - capability URL로 서빙한다. `nosniff`와 sandbox CSP를 붙인다.
  - 업로드 관리자가 있다.
- [x] 공유와 공개 뷰어:
  - `/s/<token>`은 실시간 내용을 보여주는 읽기 전용 HTML이다. 스크립트가 없다.
  - 링크마다 취소할 수 있다.
- [ ] 협업자와 권한(namespace, linkfile): 서버가 단일 사용자 구조라 범위 밖이다.
- [x] Mind map 뷰(읽기 전용), Article 레이아웃, RTL.
- [x] 캘린더: ICS 구독 피드로 대신했다. Google Calendar의 "URL로 추가"에서 쓴다. Google API 연동은 하지 않는다.
- [ ] 이메일로 inbox에 보내기: 메일 수신 인프라가 필요해 범위 밖이다.
- [ ] Dropbox/Drive 자동 백업: 서버 쪽 일일 백업과 앱의 백업 다운로드로 대신한다. 필요하면 rclone으로 `backups/`를 동기화한다.
- [x] 온보딩 튜토리얼과 What's new: 메인 메뉴에서 연다. QA에 방해되지 않게 자동으로 열지 않는다.
- [x] 읽기 전용 잠금 모드: 문서별 보기 옵션에서 켠다.

### 4. 모바일·macOS: Rust 네이티브 앱 (`native/`)
- [x] Capacitor 셸(`apps/android`) 제거 (2026-11-13).
- [x] Tauri 셸(`apps/shell`, `shell.yml`) 제거 (2026-11-13). 네이티브 앱이 나올 때까지 macOS와 Android는 웹(PWA)을 쓴다.
- **방향:** 웹(`apps/web`)과 서버는 TS 그대로 둔다. Android와 macOS는 Rust 앱으로 새로 만들고, 기존 서버 동기화 프로토콜과 호환한다.
- **UI 스택:** [NSH](https://github.com/longbridge/nsh) 0.5.0 (crates.io, `=0.5.0`으로 고정).
  - macOS: NSH의 GPUI 백엔드(`nsh-platform-gpui`)로 그린다. `crates/outliner-mac`이 창을 띄운다.
  - Android: NSH의 Android 백엔드. 입력란은 실제 `EditText`라 한글 IME가 OS 그대로 동작한다.
  - Exact(expo/exact), gpui-mobile도 검토했다. Exact는 macOS가 AppKit뿐이고 Android 지원이 초기 단계라서, gpui-mobile은 Android에서 IME 조합 입력이 없어서 제외했다.
- **구조:**
  - `crates/outliner-core`: `packages/core`의 Rust 이식. 지금은 tree, item, ids, session(undo, sync), history, Tx, commands,
    view, inline, dates. `fixtures/gen*.ts`가 TS 코어의 동작을 무작위 시나리오로 기록하고 `tests/*.rs`가 대조한다
    (CI `native-core`). 날짜 사례는 시간대 5곳(UTC, 뉴욕, 서울, 산티아고, 로드하우)에서 기록해 서머타임 경계까지 대조한다.
  - `crates/outliner-app`: NSH UI. 가상 리스트, 인라인 서식 행, 편집 중인 행만 네이티브 입력란, 키 바인딩(`When::Always`), 터치용 툴바.
    헤드리스 테스트 `tests/flows.rs` (1만 개 행 문서).
  - `crates/outliner-mac`: macOS 실행 파일 (GPUI).
  - CI `.github/workflows/native.yml`: `Outliner-macos.zip`(ad-hoc 서명 .app)과 `Outliner-android.apk`(debug 서명)를 artifact로 올린다.
- [x] core 1차 이식과 TS 대조 테스트.
- [x] NSH 시험용 앱: 헤드리스 테스트 통과, Linux GPUI(Xvfb)에서 렌더링·클릭·입력·↓ 이동 확인 (이후 Xvfb 대신 아래 결정론적 검사를 쓴다).
- [x] 외형을 웹에 맞춤 (2026-11-14): Fira Sans/Mono, 18px/22px 행(30px 간격), 불릿과 들여쓰기 안내선, 제목, 코드·링크·태그·완료·체크박스·색 라벨, 상단 바, 휴대폰 하단 툴바(웹 아이콘). 남은 차이: 사이드바, 코드 칸 좌우 여백, 편집 입력란의 글꼴(GPUI), 다크 테마.
- [x] 인라인 문법·노트·번호 목록 (2026-11-14): `inline.ts`, `dates.ts` 이식(대조 테스트 약 14만 5천 건, 변이 검출 확인),
  반복 날짜 체크(`new_instance`)와 날짜 정렬 연결. 앱이 굵게·기울임·취소선·강조·코드·코드 블록·링크·자동 링크·이미지·태그·
  날짜 칩·LaTeX(원문)·이스케이프, 노트(`n`), 번호 목록(`num`)을 웹처럼 그린다(`OUTLINER_DEMO=inline`, `tests/inline.rs`).
  남은 차이: KaTeX 수식은 원문을 고정폭으로 보인다(NSH에 수식 조판이 없다), 코드 블록은 줄을 접는다(웹은 가로 스크롤)와
  구문 강조가 없다, 날짜 칩 클릭(날짜 선택기)·반복 날짜 툴팁 없음, 노트 편집은 아직 없다(Shift+Enter), 칩의 날짜 표시는
  행이 다시 그려질 때만 갱신된다, `highlight_overdue`·`display_images_inline` 설정은 기본값(끔)으로 고정이다(코드는 있다),
  `[[` 항목 링크(`refs.ts`)와 검색 강조(`o-match`)는 해당 이식과 함께 한다.
- [x] 결정론적 외형 검사 (2026-11-14): 구조 스냅샷과 GPUI 오프스크린 렌더. 고정 문서 3개(데모 첫 화면, 인라인 쇼케이스,
  항목 모양 `looks`: 제목 3단계, 완료, 체크박스, 색 라벨 6개, 접힌 항목, 4단계 깊이)를 데스크톱(1100x800)·휴대폰(390x844),
  평상시·편집 중으로 연다(`outliner_app::fixture`). 스냅샷(`tests/snapshot.rs`, 정답 `tests/snapshots/*.txt`)은 렌더 모델
  (상단 바, 제목, 행의 깊이·불릿 중심·글자 시작·위치·높이·스타일별 구간, 툴바)과 화면의 모든 뷰를 글로 적어 바이트 단위로
  비교하고(CI `native-core`), 정답이 겹치는 곳에서 `styles.css` 값을 담는지도 검사한다. 1pt 변경에 실패하고 되돌리면 통과함을 확인했다.
  오프스크린 렌더(`outliner-mac`의 `shot`)는 PNG를 만들어 macOS 작업의 artifact `Outliner-shots`로만 올린다(비교 없음).
  Linux에서도 화면 없이 동작한다(wgpu, Mesa lavapipe). 웹 대조 중 고친 차이: 코드 블록 글자색(검정 → 본문색), "Local only" 글꼴.
  남은 차이(스냅샷에 기록됨): 편집 입력란이 한 줄이라 긴 행은 가로로 밀린다(웹은 줄바꿈). → 영역 2에서 고쳤다.
- [x] 외형의 단일 소스 (2026-11-14, 영역 6): `design/`의 TOML이 두 앱 외형의 유일한 값이다. `base.toml`(모든 항목: 색,
  라벨·구문 강조 색, 글꼴, 글자 크기·줄 높이·굵기, 행·불릿·체크박스·상단 바·툴바·사이드바·문서 여백, 모서리),
  `modes/<그룹>/<모드>.toml`(테마 8개, 크기, 밀도, 글꼴, 동아시아 글꼴, 휴대폰, 인쇄), `schema.toml`(종류·범위·설명,
  문서는 생성되는 `schema.md`). `node design/build.mjs`가 웹 CSS 변수(`apps/web/src/design/tokens.css`), 웹 설정용
  `tokens.ts`, 네이티브 `outliner-app/src/design.rs`를 만들고 `--check`가 CI(`ci.yml` test)에서 생성물이 낡았는지 본다.
  `styles.css`의 `:root`·테마 블록을 없애고 색·글자·행/바 치수를 생성 변수로 바꿨다(모든 테마·크기×밀도·글꼴, 데스크톱·
  휴대폰에서 모든 요소의 계산된 스타일이 이전 빌드와 같음을 확인, 웹 QA 전체 통과). `style.rs`는 생성값만 읽는다
  (`appearance.rs`가 기본값·모드·기기·사용자 값을 겹치고 `{table.key}` 참조를 푼다).
  관계 스펙 `design/spec.json`(행 간격, 글자 안쪽 여백, 불릿 중심→글자, 단계별 들여쓰기, 제목 위치, 제목 크기, 노트 간격,
  체크박스→글자, 상단 바 높이, 색)을 테마 8개와 크기×밀도 9개 조합, 데스크톱·휴대폰에서 두 앱 모델에 검사한다(네이티브
  `the_look_keeps_the_spec`, 웹 `npm run qa:model`). 웹 측정기(`apps/web/test/model`, `qa/model.mjs`)는 네이티브 스냅샷과 같은
  `[model]`을 만들고 같은 고정 문서(`tests/snapshots/*.items.jsonl`)로 두 앱을 대조한다(CI `parity`). 대조로 찾아 고친 네이티브
  차이: 상단 바 테두리(웹은 높이 밖, 1pt), 단계당 들여쓰기 1pt(안내선 테두리), 체크박스와 글자 간격(데스크톱 2pt, 휴대폰
  28pt 터치 영역), 줄 높이 반올림. 알려진 차이(이유와 함께 `qa/lib/model.mjs` `KNOWN`): 코드 블록 구문 강조 없음, LaTeX 조판
  없음, 휴대폰에서 긴 줄이 접히는 위치(글리프 폭), 웹 `<mark>`의 검정 글자.
  사용자 외형: 같은 스키마의 TOML(바꾼 것만). 웹은 동기화되는 설정 `appearance`에 저장하고 `<html>` 인라인 변수로 즉시
  적용, 설정 > Look에서 항목별 입력과 TOML 원문 편집(줄 번호가 붙은 오류), `.toml` 내보내기/불러오기. 웹의 Custom CSS 설정은
  없앴다(이미 저장된 `custom_css` 값은 읽지도 지우지도 않는다: 처리 방법은 사용자 결정 대기). 네이티브는 같은 TOML을
  `appearance::set`으로 겹치고, `design/test/custom.toml`로 두 앱 모델이 같은지 대조한다. 네이티브 설정 화면은 영역 4.
  남은 일: `styles.css`의 웹 전용 화면(대화상자, 메뉴, 설정, 날짜 선택기, 버전 기록, 마인드맵 등)의 여백·크기 px 값
  약 420개는 아직 리터럴이다(색, 글자 크기, 모서리, 테두리, 개요·바 치수는 모두 토큰). 데모 문서(1만 행)는 웹 측정기
  대조에서 빠졌다(inline, looks만).
- [x] 현재 행 thread와 상호작용 표시 (2026-11-14, 영역 2): 편집 중인 행과 조상 행의 불릿(번호 포함)이 thread 색이 되고,
  2px 선이 조상 불릿에서 안내선을 따라 내려와 편집 중인 행의 불릿으로 둥글게 꺾여 들어간다(웹 `.o-path`/`.o-above`/
  `.o-current`, 번호 목록의 꺾임 끝 포함). 데스크톱에서 자식이 있는 항목의 불릿에 포인터를 올리면 아래(펼침)/오른쪽(접힘)
  삼각형이 된다(NSH `group_hover`). 편집 입력란의 선택 색은 강조색의 30%다. 새 토큰: `color.thread`(`--thread`, 기본
  `{color.accent}`), `color.selection_mix`, `color.current_mix`(웹 `color-mix(... calc(var(...) * 100%))`, 계산 결과가 이전과
  같음을 Chromium에서 확인), `space.fold_mark`, `space.fold_mark_h`. 편집 입력란은 이제 여러 줄이다(NSH `multiline`, 내용만큼
  높아짐): Return은 계속 행을 나누고, Shift/Alt+Return은 줄바꿈을 넣지 않는다(웹의 Shift+Enter는 노트로 이동인데 네이티브에
  노트 편집이 아직 없다). 렌더 모델에 편집 중인 행(` current`), 번호 색(`number_color`), thread 조각(`thread`: 같은 x에서
  이어지는 직선 구간을 합친 것, `elbow`: 꺾임과 끝·반지름)을 더했고, 웹 측정기가 같은 값을 의사 요소에서 재서 편집 상태
  (`inline` s0, `looks` l17)까지 대조한다. `looks` 문서에 경로 위쪽의 형제와 그 자식(l19, l20)을 넣어 직선 구간도 검사한다.
  헤드리스 테스트: 긴 행이 입력란에서 줄바꿈되고 Return이 행을 나눔, thread 조각 수. 남은 차이: hover 삼각형은 헤드리스로
  검사할 수 없다(nsh-testing에 포인터 이동 없음), ~~여러 줄 입력란에서 ↑/↓가 바로 이전/다음 행으로 간다~~(영역 3에서 고침:
  웹처럼 캐럿이 첫/마지막 줄에서 0.7줄 안에 있을 때만 행을 옮기고, 그 밖에서는 키가 입력란으로 가서 줄 사이를 오간다.
  캐럿 위치는 `on_caret_move`, 입력란 높이는 `on_layout`으로 안다), 현재 행 배경 강조(`current_highlight`)와 블록 선택(`--sel`)은 네이티브에 설정·기능이
  아직 없다, Android `EditText`의 여러 줄 + Return 제출은 실기기 확인이 필요하다.
- [x] 사이드바 (2026-11-14, 영역 3): 웹 `sidebar.ts`처럼 Files/Bookmarks/Tags 탭과 그 아래 목록. 데스크톱은 상단 바의
  사이드바 버튼으로 여닫는 왼쪽 패널(처음엔 열림, 기본 260pt, 경계의 6pt 손잡이를 끌어 160~520pt), 휴대폰은 화면의 82%(최대
  320pt) 서랍과 그 뒤의 어두운 scrim(누르면 닫힘, 다른 문서를 열어도 닫힘). 동기화 전이라 문서는 로컬에만 있다: store가 문서
  여러 개를 들고(`State::files`, 열지 않은 문서는 세션과 보기 상태를 보관), 앱은 데모와 빈 "Untitled"로 시작한다. 문서 이름은
  루트 글(웹처럼 둘이 같다). 문서 전환, 새 문서(+ 버튼: 열고 이름 편집), 이름 바꾸기(길게 누르기 또는 포인터를 올리면 보이는
  "⋯", Return/포커스 이탈은 저장, Escape는 취소, 빈 이름은 버림), 빈 문서는 제목 아래를 누르면 첫 항목이 생긴다(웹
  `focusEnd`). Tags 탭은 열린 문서와 모든 문서의 태그(`extract_tags`, 대소문자 무시 집계, 많이 쓴 순/이름순 전환, 구역 접기).
  새 토큰 23개(`space.sidebar_min` 등 사이드바 탭·머리·행·태그 행·서랍·이름 입력란 치수, `type.head_tracking`): 웹
  `styles.css`의 같은 리터럴을 토큰으로 바꿨다(값은 같다; 끌기 범위는 이제 CSS `clamp`). 렌더 모델에 사이드바(상자, 탭과 활성,
  구역 머리, 파일 행과 열린 문서 배경, 태그 행, 각 글자의 위치와 스타일)와 scrim을 더했고, 웹 측정기가 실제 `Sidebar`를 같은
  작업 공간(고정 문서 + 빈 문서)으로 띄워 같은 값을 잰다. 새 고정 상태: `looks-phone-files`(서랍), `inline-desktop-tags`,
  `inline-phone-tags`. 데스크톱 정답은 문서가 패널 오른쪽으로 옮겨져 모두 바뀌었다. 헤드리스 테스트 `tests/sidebar.rs` 7개.
  웹과 다른 점(아직 없는 기능): 폴더와 파일 메뉴(복사, 내보내기, 보관, 삭제 등), "New…" 메뉴(문서만 바로 만든다), 문서 끌어
  옮기기, 찾기·받은 편지함·모두 접기 버튼(그려만 둠), 북마크(빈 안내만), 태그 누르기(검색 없음)와 태그 메뉴, 이름 바꾸기의
  더블 클릭(NSH에 없어서 길게 누르기), 서랍 여닫기 애니메이션, 정렬 설정·사이드바 너비의 저장.
- [x] 테마와 표시 설정 (2026-11-14, 영역 4): 설정 화면(데스크톱은 상단 바 톱니, 휴대폰은 상단 바 메뉴 > Settings)은 웹 설정
  대화상자에서 네이티브에 있는 탭만 둔다. Preferences: 테마(Auto-detect 기본: 시스템의 밝음/어둠을 따르고 바뀌면 바로 반영,
  웹의 테마 7개), "Use true black (dark theme)"(어두운 테마를 `black`으로), 목록 밀도, 글자 크기(소/중/대), 글꼴, 동아시아
  글꼴. 웹의 `<select>` 대신 선택지를 버튼으로 모두 보인다. Look: 사용자 외형 TOML 원문(입력하는 동안 검사해 "Line N: ..."
  오류와 빨간 테두리를 보이고 Apply로 적용, 잘못된 TOML은 외형을 바꾸지 않음), 스키마에서 만든 항목별 입력(표마다 접기,
  색은 견본과 함께, 선택형은 버튼, Return이나 포커스 이탈로 적용, 범위 밖 값은 항목 이름과 함께 거절, ×로 테마 값으로 되돌림),
  `.toml` 내보내기(데스크톱은 저장 대화상자, 휴대폰은 저장 대화상자가 없어 클립보드로 복사)와 불러오기(파일 선택기). 선택은 기기의
  NSH storage에 웹과 같은 이름(`theme`, `use_true_black`, `font_size`, `density`, `font`, `eastern_asian_font`,
  `appearance`)으로 저장해 재시작 후에도 유지한다(기본값은 저장하지 않고, 선택지에 없는 값과 잘못된 TOML은 없는 것으로 본다).
  외형이 바뀌면 앱 루트가 화면을 다시 만든다(`lib.rs` `root`: 뷰는 외형을 값으로 읽고, 보여 줄 상태는 store와 전역 signal에
  있다). 새 토큰 31개(설정 대화상자·필드·버튼·탭·목록 행·TOML 입력란·메뉴 치수, `type.modal_title`): 웹 `styles.css`의 같은
  리터럴을 토큰으로 바꿨다(값은 같다). 테스트: 헤드리스 `tests/settings.rs` 6개(시스템 외형 따르기와 true black, 선택 즉시 적용과
  재시작 후 유지, 휴대폰 메뉴, TOML 오류 줄과 적용, 항목 입력과 되돌리기, 내보내기/불러오기), 구조 스냅샷에 대표 조합 5개
  (`design/test/combos.json`: 테마·크기·밀도·글꼴·사용자 TOML, `qa:model`이 웹에서 같은 조합을 재서 대조)와 설정 화면 4개
  (`looks-{desktop,phone}-{settings,look}`). 앱 루트에 뷰가 하나 더 생겨 모든 정답의 `[views]` 들여쓰기가 바뀌었다(`[model]`은
  같음). 흔들리던 동작 테스트: flows/inline/sidebar 테스트가 1만 행 데모 대신 `fixture`의 120행 데모를 연다(1만 행은 스크롤
  성능 테스트 하나만). 웹과 다른 점: TOML 원문은 입력 중 자동 적용 대신 Apply(입력란이 다시 만들어져 포커스를 잃지 않게),
  오류 줄은 메시지와 테두리로만 표시(입력란 안의 줄 선택은 없음), 대화상자 그림자는 사이드바 서랍의 흐림 값을 쓴다,
  설정의 다른 항목(Keymap, Sync & data, 날짜, 입력 등)은 아직 없다.
- [ ] 실기기 확인 (사용자): macOS 앱과 Android APK에서 한글 입력, 키 바인딩(↑/↓, Tab/Shift+Tab, ⌘Z), 스크롤 성능, 메모리.
- [ ] 남은 core 이식: `search.ts`, `paste.ts`, `export.ts`(HTML 렌더러 포함), `refs.ts`, `calendar.ts`.
- [ ] 서버 동기화 연결(HTTP/WS 클라이언트, 로그인), 문서 목록, 줌, 검색.
- [ ] 알려진 제약: Rust 문자열은 짝이 맞지 않는 UTF-16 서로게이트를 담을 수 없다. 웹이 이런 문자열을 보내면 역직렬화가 실패한다.
- 명령:
  ```sh
  for g in gen gen-commands gen-inline; do node native/crates/outliner-core/fixtures/$g.ts; done
  (cd native && cargo test)                                                      # core 대조 + 앱 헤드리스 테스트
  (cd native && UPDATE_SNAPSHOTS=1 cargo test -p outliner-app --test snapshot)   # 구조 스냅샷 정답 갱신 (커밋에 이유)
  node design/build.mjs                                                          # design/*.toml → CSS 변수, tokens.ts, design.rs
  npm run qa:model                                                               # 웹 모델 ↔ 네이티브 스냅샷 대조, spec.json
  (cd native && cargo run -p outliner-mac --features shot --bin shot -- /tmp/shots)  # 오프스크린 PNG
  cd native && cargo run -p outliner-mac                                         # 데스크톱 (Linux는 GPUI 개발 라이브러리 필요)
  cd native/crates/outliner-app && nsh package --target android                  # APK (Android SDK, NDK 필요)
  ```

## 자주 쓰는 명령

```sh
npm test && npm run typecheck                    # core/server 테스트
npm run qa:serve && npm run qa                   # Dynalist 비교(오프라인 녹화본): editing/fuzz/render/layout/layout-mobile/menus
npm run qa:live                                  # 라이브 Dynalist와 나란히
node qa/run.mjs editing --only=undo/ --verbose   # 일부만, 차이를 바로 출력
node qa/show.mjs fuzz fuzz/123                   # 차이를 나란히 보기 (단계별)
FUZZ_FROM=5001 FUZZ_N=200 node qa/run.mjs fuzz   # 새 시드로 탐색 (라이브)
apps/web/e2e/snapshot.sh && node apps/web/e2e/keys.mjs   # 우리 쪽 e2e (popups/drag/mobile/features/server-extras/sync도 있음)
npm run build -w @outliner/web && systemctl --user restart outliner   # 배포 갱신
```

- **QA 리포트:** `qa/out/<suite>/report.html`. gitignore 대상이고, CI에서는 artifact로 올라간다.
- **알아둘 함정:**
  - `pkill -f`의 패턴이 자기 명령줄에도 들어 있으면 자기 셸까지 죽인다.
  - Dynalist는 키 입력 간격이 60ms보다 짧으면 순서가 꼬인다. 부하가 걸리면 undo 묶음 타이밍이 흔들리는데, 러너가 단독 재실행으로 걸러준다.
