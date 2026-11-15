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
    view, inline, dates, search, refs, paste, export(HTML 렌더러 포함), calendar. `fixtures/gen*.ts`가 TS 코어의 동작을 무작위 시나리오로 기록하고 `tests/*.rs`가 대조한다
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
  구문 강조가 없다, 날짜 칩 클릭(날짜 선택기)·반복 날짜 툴팁 없음, ~~노트 편집은 아직 없다(Shift+Enter)~~(영역 9), 칩의 날짜 표시는
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
  없음, 휴대폰에서 긴 줄이 접히는 위치(글리프 폭), ~~웹 `<mark>`의 검정 글자~~(영역 8에서 고침).
  사용자 외형: 같은 스키마의 TOML(바꾼 것만). 웹은 동기화되는 설정 `appearance`에 저장하고 `<html>` 인라인 변수로 즉시
  적용, 설정 > Look에서 항목별 입력과 TOML 원문 편집(줄 번호가 붙은 오류), `.toml` 내보내기/불러오기. 웹의 Custom CSS 설정은
  없앴다(이미 저장된 `custom_css` 값은 읽지도 지우지도 않는다: 영역 8에서 사용자 결정대로 보존하고 무시한다). 네이티브는 같은 TOML을
  `appearance::set`으로 겹치고, `design/test/custom.toml`로 두 앱 모델이 같은지 대조한다. 네이티브 설정 화면은 영역 4.
  남은 일: ~~`styles.css`의 웹 전용 화면 px 리터럴~~(영역 8에서 토큰으로 옮김). 데모 문서(1만 행)는 웹 측정기
  대조에서 빠졌다(inline, looks만).
- [x] 웹 정리 (2026-11-15, 영역 8): (1) 저장된 `custom_css`는 지우지 않고 무시한다. 설정 > Preferences > Advanced의 예전
  Custom CSS 자리에 더 이상 지원하지 않는다는 안내(값이 저장돼 있으면 보존하지만 적용하지 않는다는 말)와 Look 탭으로 가는
  링크를 둔다. (2) `==강조==` 글자는 행의 글자색을 따른다(`.o-highlight, .o-mark { color: inherit }`, 브라우저 `<mark>`의
  검정 대신): 네이티브와 같아져 `qa/lib/model.mjs` `KNOWN`의 해당 항목을 지웠다. (3) `styles.css`의 남은 px 리터럴을
  `design/`으로 옮겼다: 간격 척도 `space.step1`~`step12`(`--step-1`~`--step-12`: 1, 2, 3, 4, 6, 8, 10, 12, 16, 20, 24, 30)와
  척도 밖 값의 이름 붙은 크기(`space.search_box`, `space.datepicker`, `space.history`, `space.help` 등 48개,
  `type.ui_mini`/`ui_large`/`ui_head`/`ui_heading`, `type.finder`, `type.mm*` 등 11개, `radius.backlinks`). 음수는
  `calc(-1 * var(...))`. 남은 px는 `@media (max-width: 760px)`(CSS 변수를 미디어 조건에 쓸 수 없음, 값은 `design/modes.toml`의
  `device.media.phone`)와 주석뿐이다. 모든 규칙의 선언값(테마 8개, 크기×밀도 9개, 화면·인쇄, 데스크톱·휴대폰)과 첫 화면·설정
  탭 4개의 모든 요소 계산 스타일이 이전 빌드와 같음을 확인했다(Look 탭에 새로 생긴 항목 행의 글자 폭만 다름).
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
  웹과 다른 점(아직 없는 기능): ~~폴더와 파일 메뉴(복사, 내보내기, 보관, 삭제 등), "New…" 메뉴(문서만 바로 만든다), 문서 끌어
  옮기기~~(영역 11), ~~찾기·받은 편지함 버튼(그려만 둠)~~(영역 14), ~~모두 접기 버튼, 북마크(빈 안내만)~~(영역 11), 태그 누르기(검색 없음)와 태그 메뉴, ~~이름 바꾸기의
  더블 클릭(NSH에 없어서 길게 누르기)~~(영역 11: NSH `PressEvent::clicks`로 더블 클릭), 서랍 여닫기 애니메이션, ~~정렬 설정·사이드바 너비의 저장~~(영역 11).
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
  ~~오류 줄은 메시지와 테두리로만 표시~~(영역 9: 포커스 이탈·Apply에서 오류 줄 선택, 올바르면 적용), 대화상자 그림자는 사이드바 서랍의 흐림 값을 쓴다,
  설정의 다른 항목(Keymap, Sync & data, 날짜, 입력 등)은 아직 없다.
- [x] 편집 기능 (2026-11-15, 영역 9): 노트, 블록 선택, 날짜 선택기, Look 탭 결정 반영.
  노트: 항목 노트(`n`)를 내용 아래의 자체 입력란(노트 글꼴, 여러 줄, Return은 줄바꿈)에서 편집한다. Shift+Enter가 내용↔노트를
  오가고(웹 `goto-note`: 노트가 빈 항목에서 내용의 선택한 글은 노트로 옮겨짐), 노트 첫 줄의 ↑는 내용 끝, 마지막 줄의 ↓는 다음
  행, 노트 맨 앞의 ←와 빈 노트의 Backspace는 내용 끝으로 간다. 노트의 Tab은 탭 문자를 넣는다. 노트를 누르면 노트 편집, 휴대폰
  툴바의 노트 버튼은 `goto-note`이고 노트를 편집하는 동안 툴바는 숨는다(웹 `MobileBar.update`). 빈 노트는 줄을 차지하지 않는다.
  undo는 캐럿 또는 선택한 항목을 되돌린다(`store::Mark`). 블록 선택: Shift+↑/↓(입력란의 첫/마지막 줄 또는 글 끝에서 항목 선택,
  선택 중에는 형제로 넓히고 부모/다음 항목으로 나감), Mod+Shift+A(보기의 최상위 전부), 선택 중 Mod+A(형제 전부, 다음은 부모),
  Escape(해제, 한 번 더 누르면 첫 항목 맨 앞을 편집; 편집 중 Escape는 편집을 멈추고 다시 누르면 돌아감), ↑/↓/←/→로 편집 복귀,
  Backspace/Delete/Mod+Shift+Backspace 삭제, Return은 빈 항목 하나로 바꿈, Tab/Shift+Tab, Mod+↑/↓, Mod+Enter, Mod+Shift+C,
  Mod+./Mod+Shift+., Mod+], undo/redo, Mod+C/Mod+X(웹 `portableToPlain`과 같은 평문). 선택 표시는 `--sel` 배경(항목 행은 불릿부터
  둥글게, 아래 행은 자식 안내선부터, 그 안내선은 투명). 휴대폰 상단 바의 "Select multiple items"가 선택 모드(누를 때마다 항목을
  넣고 뺌, 켜진 동안 버튼은 `topbar_fg_on` 색)를 켜고, 툴바는 선택 중에도 보이며 여러 항목이면 한 항목용 버튼(확대, 노트, 날짜)이
  숨는다. 날짜 선택기(`datepicker.rs`, 웹 `datepicker.ts`): Mod+D, 휴대폰 툴바의 날짜 버튼, 공백 뒤에 친 `!`로 연다. 달력(이전/
  다음 달, 오늘·선택·범위 표시), 요약, Time(웹과 같은 해석: "3pm", "1530", "noon", "All day" 등, 잘못되면 오류), 날짜 범위와
  End time, 반복(미리 정한 것 + 사용자 정의: 간격·단위·요일), From completion, Delete date/Cancel/Done. 키: Return=Done,
  Escape=Cancel, 화살표=하루/일주일, PageUp/Down=한 달. 캐럿이 날짜 안이면 그 날짜를 고치고, 아니면 단어 뒤에 공백을 맞춰 넣는다.
  Look 탭(영역 4 결정): Apply 유지 + TOML 입력란에서 포커스가 나가면 올바른 TOML은 적용, 오류면 오류 줄을 입력란에서 선택
  (`ElementRef::set_selection`). 휴대폰 내보내기는 NSH 0.5.0에 공유 시트가 없어 클립보드 그대로다.
  테스트: 헤드리스 `tests/editing.rs` 11개(노트 오가기·경계·빈 노트·Tab, 휴대폰 노트 버튼, Shift+↓/↑·Escape·Mod+Shift+A,
  선택 항목에 Tab·Backspace·undo·Return, 복사, 휴대폰 선택 모드, 날짜 넣기·고치기·시간 오류·삭제, 선택기 키·반복·범위·취소,
  `!` 입력, 휴대폰 날짜 버튼), `tests/settings.rs`에 포커스 이탈 적용 1개. 선택기의 치수는 웹과 같은 토큰(`space.datepicker`,
  `space.dp_*`, `type.dp_nav`, `space.stepN`)을 읽는다. 구조 스냅샷에 고정 상태 6개(`looks`/`inline`의 노트 편집,
  `looks`의 l6 선택, 데스크톱·휴대폰): 렌더 모델에 노트 입력란(`note` 아래 `input`)과 ` selected=<색>`을 더했고 웹 측정기와
  `qa/model.mjs`가 같은 상태를 재서 대조한다(차이 없음, 기존 알려진 차이만). 기존 정답은 바뀌지 않았다.
  웹과 다른 점: ~~선택기는 캐럿 옆 팝오버가 아니라 데스크톱은 가운데~~(영역 12에서 캐럿 아래 팝오버), 휴대폰은 아래 시트이고, 여는 동안 행 입력란이 닫힌다(닫으면
  캐럿이 돌아옴). ~~주 시작 요일(일요일)과 12시간제는 웹 기본값으로 고정~~(영역 12에서 설정). 아직 없는 것: ~~날짜 칩을 눌러 여는
  것, 데스크톱 Shift/Mod+클릭과 끌어서 선택, 선택 항목 드래그, 서식 키(Mod+B 등), 제목(줌 루트)의 노트 편집~~(영역 13). TOML 오류 줄 선택은
  헤드리스로 확인할 수 없다(nsh-testing 0.5.0에 선택 읽기 없음; 줄 계산만 단위 테스트). 웹 버그(~~고치지 않음~~ 영역 12에서 고침): 범위가 아닐 때 PageUp/Down 뒤 Done을 누르면 이전 끝 날짜가
  시작으로 바뀌어 들어간다(`datepicker.ts` `done`의 swap); 네이티브는 범위가 아니면 끝을 시작과 함께 옮긴다.
- [x] 나머지 설정 (2026-11-15, 영역 12): 설정 화면에 Keymap, Sync & data 탭을 더하고 Preferences를 웹의 구역대로 채웠다.
  Preferences: 네이티브가 반영하는 웹 설정만 웹과 같은 이름·기본값으로 둔다. Appearance(현재 항목 강조 `current_highlight`,
  `bullet_threading`, 좁은 문서 `center_doc`(`space.doc_max`), `display_images_inline`), Viewing(`show_checked`: 모든 문서의 보기,
  `show_notes`: 보임/첫 줄(한 줄 말줄임, 줄바꿈은 공백)/숨김), Input(`word_count`: 오른쪽 아래 상태 막대, 웹과 같은 낱말 규칙,
  `workflowy_shortcuts`), Date(`time_format`, `start_day`, 날짜 표시 형식 2개, `show_timezone`, `highlight_overdue`,
  `recurrence_check_behavior`), Control(`use_bullet_to_zoom`, `unindent_in_place`), Advanced(`auto_focus_first_item`,
  `dyslexia_font`: 글꼴 모드 `dyslexic`). 저장은 NSH storage에 웹 이름으로(참/거짓은 `true`/`false`, 키맵은 JSON)
  하며, 바뀐 설정만 기록한다: 이 앱이 모르거나 읽지 못하는 저장값(예: `tag_bg`, 잘못된 값)은 지우지 않는다. 외형이 아닌 설정이
  바뀌면 개요·키·상태 막대만 다시 만든다(`lib.rs` `behaving`; 설정 화면은 그대로라 스크롤이 유지된다).
  Keymap: 웹 `keymap.ts`의 명령 90개·기본 키·WorkFlowy 키·`Keymap`(JS Map과 같은 덮어쓰기)을 `keymap.rs`로 옮기고, 단위
  테스트가 `keymap.ts` 원문을 읽어 명령 목록이 같은지 확인한다. 편집기(`settings::keymap_pane`)는 검색, 키 칩(×로 제거), +로 녹화(수정
  키가 있는 조합과 F키를 모두 듣다가 처음 눌린 것을 넣고 그 키를 가졌던 명령에서 뺀다, Escape로 중단, 플랫폼 예약 키 제외),
  명령별 "Reset to default", "Reset all shortcuts". 고정 명령(undo, redo, select-up/down)은 "fixed". 저장은 `shortcut_override`.
  데스크톱 키 바인딩(편집 중 `item_keys`, 선택 중 `block_keys`, 앱 전체 `app_keys`)이 이 키맵을 읽는다. 네이티브에 동작이 있는
  명령만 묶인다(들여쓰기·내어쓰기·접기·확대/축소·삭제·노트 이동·완료·체크박스·이동·전체 선택·undo/redo·설정 열기·사이드바·
  완료 항목/노트 보이기).
  Sync & data: 동기화 서버 칸은 웹의 서버 없는 화면(URL, 비밀번호, Connect)만 두고 Connect는 "아직 없음"을 알린다. Backup은
  웹 `backupZip`과 같은 zip(문서마다 `<제목>.txt`와 `.opml`, 대소문자 무시 고유 이름, 저장 방식 zip 작성기 `backup.rs`)을
  데스크톱은 저장 대화상자로, 휴대폰은 NSH에 저장 대화상자가 없어 앱 데이터 폴더(`backups/`)에 저장하고 경로를 보인다.
  Import는 OPML/텍스트 파일(여러 개)을 문서마다 하나씩 만들고 마지막 문서를 연다(`paste`/`export` 이식).
  날짜 선택기(영역 9 결정): 데스크톱은 캐럿 아래 팝오버(웹 `placePopup`과 같은 규칙: 아래 공간이 모자라고 위가 더 넓으면 위,
  화면 안으로 좌우 조정, 공간이 모자라면 높이를 줄이고 스크롤, 최소 높이는 새 토큰 `space.popup_room`(80, 웹 `position.ts`도
  CSS로 읽음)). 캐럿 위치는 입력란의 `on_caret_move` `page_bounds`. 캐럿 위치를 모를 때(항목 선택 중)는 가운데. 휴대폰은
  아래 시트 그대로. 주 시작 요일·12/24시간은 설정을 따른다(달력, 시간 칸, 날짜 칩), 새 날짜는 `show_timezone`이면 시간대를 붙인다.
  웹 버그 수정: `datepicker.ts`에서 범위가 아닐 때 PageUp/Down이 끝 날짜를 시작과 함께 옮긴다(`pageMonth`, vitest
  `apps/web/test/ui/datepicker.test.ts`).
  테스트: 헤드리스 `tests/prefs.rs` 8개(노트 숨김/첫 줄·완료 항목 숨김과 재시작 후 유지·모르는 저장값 보존, 24시간·표시 형식·
  월요일 시작, 팝오버 위치(데스크톱 캐럿 아래, 휴대폰 아래 시트), 불릿 확대·제자리 내어쓰기, 키 녹화·제거·새 키로 완료·전체 초기화,
  WorkFlowy 키, 백업 zip·가져오기·동기화 안내, 낱말 수·첫 항목 자동 편집), 단위 테스트(키맵, zip/CRC, 파일 이름, 가져오기).
  구조 스냅샷에 고정 상태 8개(`looks-{desktop,phone}-{keymap,data}`, `{looks,inline}-{desktop,phone}-prefs`: `fixture::PREFS`의
  완료 숨김·노트 첫 줄·thread 끔·현재 항목 강조·24시간)를 더했고 `qa:model`이 웹에서 같은 설정으로 `prefs` 상태를 재서 대조한다
  (휴대폰 편집 행 줄바꿈만 기존과 같은 이유의 알려진 차이). 설정 화면 정답 4개는 탭 2개와 Preferences 항목이 늘어 바뀌었다.
  웹과 다른 점·아직 없는 것: 웹 설정 중 네이티브에 해당 기능이 없는 것은 화면에 두지 않았다(오른쪽에서 왼쪽 쓰기(NSH 레이아웃 방향은
  있으나 안내선·thread가 왼쪽 기준), 맞춤법 검사, 모든 문서의 태그 자동 완성, 백링크 표시, 받은 편지함, 항목 hover 툴바, 검색 팁,
  "Move item to" 위치, 개행 정리 메뉴, 보이는 항목만 내보내기, 태그 배경(`tag_bg`: NSH 글 구간에 안쪽 여백·테두리가 없음),
  문서 테두리(`doc_border`), 동기화와 서버 부가 기능). 키 녹화에서 수정 키 없는 키는 무시된다(웹은 안내 토스트). 키 바인딩은
  데스크톱만(NSH가 휴대폰에서 키를 보내지 않음). ~~웹의 WorkFlowy 확대/축소 키(Alt+→/←)는 같은 키의 Back/Forward가 목록 뒤에 있어
  덮인다~~(영역 13에서 웹과 함께 고침: WorkFlowy 키가 이긴다). ~~날짜 칩을 눌러 여는 선택기~~(영역 13), 항목 선택 중 팝오버 위치는 아직 없다.
  ~~항목 hover 툴바, "Move item to" 위치~~(영역 13에서 더함).
- [x] 파일·폴더·북마크와 기기 저장 (2026-11-15, 영역 11): 모두 로컬이다(서버 동기화는 범위 밖).
  저장: 웹의 로컬 형식 그대로 문서마다 `DocSnapshot`(version, confirmed, pending)을 NSH storage의 `doc:<id>`에, 파일 트리·
  북마크를 특수 문서 `workspace`(`files`/`settings`/`bookmarks` 노드, 웹 `workspace.ts`와 같은 구조와 속성: `kind`, `title`,
  `archived`, 접힘, 북마크의 `doc`/`zoom`)로 `doc:workspace`에 둔다(`workspace.rs`). 변경 300ms 뒤 저장(웹 `save` 타이머), 앱이
  배경으로 가면 바로 저장, pending은 서버로 보낼 때처럼 `compact`한다. 마지막으로 연 문서(`outliner:lastDoc`)와 문서별 줌
  (`outliner:zooms`)도 웹의 키로 저장해 다시 열 때 그대로 연다. 처음 실행하면 데모와 빈 문서를 만들어 저장한다. 문서 이름은 루트
  글이고, 저장할 때 파일 항목의 제목을 맞춘다(웹 `syncTitleFromDoc`).
  폴더: "New…" 메뉴(New document, New folder, Import OPML…: 영역 12의 가져오기로 그 폴더에 문서를 만든다), 폴더 메뉴의 같은 항목, 이름 바꾸기(더블 클릭 또는 메뉴, 행에서 편집),
  누르면 접기/펴기(▸/▾), 모두 접기 버튼, 들여쓰기(`space.file_indent`). 파일 메뉴(오른쪽 클릭, 포인터를 올리면 보이는 "⋯",
  길게 누르기): 문서는 Make a copy(바로 뒤에 "(copy)", 항목·노트 복사 후 열기), Add to bookmarks, Archive/Unarchive(행 투명도
  `color.archived_opacity`), Rename, Delete(웹과 같은 확인 대화상자). 폴더를 지우면 안의 문서도 목록에서 빠진다. 지운 문서의 데이터는
  웹처럼 기기에 남는다. 팝업 메뉴는 웹 `overlay.ts`의 `.menu`를 따른 `menu.rs`(화면 안에 맞춤, 바깥 누르기나 Escape로 닫기), 확인과
  북마크 이름 대화상자는 영역 10의 `dialog.rs`, "Copy created"/"Bookmarked" 알림은 `toast.rs`를 쓴다. 영역 12의 가져오기
  (Sync & data)도 이제 파일 트리에 항목을 만든다(`workspace::import`).
  끌기(데스크톱): 행을 끌면 아래 행의 위/아래 1/2(폴더는 가운데 절반이면 안으로)에 놓이고, 놓일 자리를 강조색 2pt 선이나 선택색
  배경으로 보인다(웹 `.drop-before/after/into`). NSH `on_touch` + `claim_touches(on_move)`로 구현했고 헤드리스 `drag`로 검사한다.
  휴대폰은 행 끌기 대신 파일 메뉴의 "Move to…"(최상위 + 폴더 목록, 자기 자신과 그 안은 빠짐)로 옮긴다(웹도 터치 끌기가 없다).
  북마크: 별 버튼이 지금 보기(문서, 줌 중이면 그 항목: 이름은 항목 글의 평문)를 북마크하고 Bookmarks 탭을 연다. 누르면 그 문서를
  그 줌으로 연다(검색 북마크는 검색이 생길 때까지 문서만 연다). 더블 클릭/메뉴의 Rename은 대화상자, Remove와 "×"는 확인 후 지움,
  데스크톱은 끌어서 순서 변경. 사이드바 상태: 태그 정렬은 웹 pref 이름 `tag_sort`(`alpha`/`count`)로, 웹이 저장하지 않는 열림·탭·
  너비는 `sidebar_open`/`sidebar_tab`/`sidebar_width`로 기기에 저장해 다시 시작해도 유지한다.
  새 토큰: `color.archived_opacity`(웹 `.file-row.archived`의 .6, `styles.css`의 리터럴을 이 토큰으로 바꿨다; 값은 같다).
  아이콘 `zoomin`(웹 것) 추가.
  테스트: 헤드리스 `tests/files.rs` 10개(폴더 메뉴의 OPML 가져오기, 저장과 재시작 후 복원·마지막 문서·줌, 폴더 만들기·이름·넣기·접기·모두 접기와 저장, 더블
  클릭 이름 바꾸기·복사·보관·삭제 확인과 취소·열린 문서 삭제 후 다음 문서, 폴더 삭제, 데스크톱 끌기로 순서·폴더 안·밖, 휴대폰
  Move to…, 북마크 만들기·열기·이름 바꾸기·끌기·지우기·저장, 사이드바 상태 저장). 수동 시계라서 저장 타이머가 결정론적으로 돈다.
  `tests/sidebar.rs`는 + 버튼과 길게 누르기가 메뉴를 거치도록 바꿨다. 구조 스냅샷에 고정 상태 6개(`looks-{desktop,phone}-
  {folders,menu,marks}`: 폴더 두 개(열림·접힘)와 보관한 문서, 폴더 메뉴, 북마크 두 개)를 더했고, 렌더 모델에 파일 행의 접힘 표시
  (`tog`)·투명도(`opacity`)와 팝업 메뉴(`menu`, `item`, `sep`와 글자)를 더했다. 웹 측정기와 `qa/model.mjs`가 같은 상태를 재서
  대조한다(차이 없음; 알려진 차이 1개: 휴대폰 메뉴의 "Move to…").
  비교기가 메뉴 항목 높이·글자 위치와 투명도 차이를 잡는 것을 정답을 바꿔 확인했다. 기존 정답은 `[views]`만 바뀌었다(파일 행을
  감싸던 뷰가 없어지고 이름 글자에 test id `file-name`).
  웹과 다른 점: 파일 메뉴에 Version history, Share…, Export…, Clean up newlines가 없다(서버가 없고, 내보내기는 영역 10의
  화면 메뉴에 있다). 문서가 하나도 남지 않으면 웹은 빈 화면을 보이지만 네이티브는 새 "Untitled"를 만든다(store가 늘 문서 하나를 연다).
  메뉴 위치는 누른 곳(웹과 같음)이고 휴대폰도 같은 팝업이다. ~~편집기 항목을 사이드바 문서로 끌어 옮기기~~(영역 15), 빈 곳 오른쪽 클릭의 범위
  (네이티브는 파일 목록 영역만), 끌 때 따라오는 행 그림은 없다. 저장소는 NSH storage(앱당 5MB 한도)라 아주 큰 문서는 저장에
  실패할 수 있다(오류를 로그로 남기고 기존 저장본은 그대로 둔다). 서랍 여닫기 애니메이션은 아직 없다(NSH `Motion::entering`으로
  여는 쪽은 가능하지만 닫는 쪽은 뷰를 바로 내리므로 함께 미뤘다).
  웹 버그(~~고치지 않음~~ 영역 15에서 고침): 파일 행을 같은 폴더 안에서 아래로 끌면 한 칸 더 내려간다(`sidebar.ts` 파일 `drop`이 끄는 행을 뺀 뒤의
  위치를 보정하지 않음; 북마크 쪽은 보정한다). 네이티브는 보정해서 놓은 자리에 들어간다.
- [ ] 실기기 확인 (사용자): macOS 앱과 Android APK에서 한글 입력, 키 바인딩(↑/↓, Tab/Shift+Tab, ⌘Z), 스크롤 성능, 메모리.
- [x] 남은 core 이식 (2026-11-15, 영역 7): `search.ts`(쿼리 언어, `searchTree`, 연산자 시간 값, 낱말 일치, 퍼지 일치),
  `refs.ts`(`parseItemUrl`, 백링크와 개수, 찾아 바꾸기), `paste.ts`(들여쓴 텍스트, 엔티티, 마크업 토크나이저, HTML → 텍스트,
  OPML, 클립보드 우선순위), `export.ts`(텍스트·HTML·OPML, `inline.ts`의 HTML 렌더러 `render.rs`), `calendar.ts`(ICS).
  모듈마다 생성기(`fixtures/gen-{search,refs,paste,export,calendar}.ts`, 공용 `fixtures/lib.ts`)와 대조 테스트
  (`tests/{search,refs,paste,export,calendar}.rs`)를 두었다. 대조 약 4만 6천 건(search 23,420, paste 14,110, export 6,000,
  refs 2,640, calendar 150(피드 전체 비교, 일정 약 1,500개)), 검색과 렌더러는 시간대 여러 곳에서 기록했다. 변이 25개를 모두
  검출했다. 웹과 다른 점: 검색 연산자 시간의 `Date.parse` 대체 경로는 ISO 8601과 `2026/10/22`, `10/22/2026` 꼴만 읽는다
  (V8은 `1`, `next 3`도 날짜로 읽는다). TS가 일반 객체로 찾아서 생기는 버그(엔티티·속성 이름이 `constructor`, `toString` 같은
  `Object.prototype` 멤버일 때)는 옮기지 않았다. 앱 연결(검색 화면, `[[` 링크, 붙여넣기, 내보내기)은 영역 9/10에서 한다.
- [x] 검색, 항목 링크, 붙여넣기, 내보내기 (2026-11-15, 영역 10): 영역 7의 core 이식을 앱에 연결했다(`find.rs`, `links.rs`,
  `export.rs`, 공용 대화상자 `dialog.rs`, 알림 `toast.rs`). 검색: 상단 바의 검색 버튼(데스크톱·휴대폰)과 Mod+F가 문서 위에 웹의
  검색 상자를 연다(데스크톱 320pt, 휴대폰은 문서 폭). 입력하는 동안 `compile_query`(입력 중이면 마지막 낱말은 접두어)와
  `search_tree`로 걸러 일치 항목과 조상만 보이고(보기의 filter), 일치 항목의 일치 낱말에 `.o-match` 배경을 칠한다(본문, 노트, 코드,
  태그, 링크 글자; URL 안에서 일치하면 링크 원문을 드러냄). Escape와 닫기 버튼은 검색을 끝내고 열기 전의 캐럿·선택으로 돌아간다.
  검색 팁(`show_search_tips`, 기본 켬)에서 Flat search(Shift+Enter)와 Search in all documents(Mod+Enter, 메뉴의 "Search everywhere")가
  결과 화면을 연다: 결과 수, 체크 필터로 숨은 수, 정렬(`sort_comparator`, 웹의 `<select>` 대신 버튼), "Go back [Esc]", 문서별
  카드(전체 검색), 결과를 누르면 그 문서를 열고 항목을 편집한다(`revealItem`). 태그를 누르면 검색어에 넣거나 뺀다. 다른 문서를 열면
  검색이 끝난다. 찾아 바꾸기: 메뉴의 "Search and replace"(Find, Match case, Replace with, 줌 범위의 일치 수, Replace all은 undo 한 번,
  결과는 알림). 항목 링크: 공백 뒤 `[[`가 모든 문서의 항목(열린 문서 먼저)을 `fuzzy_filter`로 제안하고(10개, 일치 글자 강조, 경로),
  `#`/`@`가 쓰인 태그를 제안한다(8개, 개수; `global_tag_autocomplete`면 모든 문서). ↑/↓, Return/Tab, 누르기로 고르고 Escape는 목록만
  닫는다. 링크는 웹 형식 `[제목](#d=<문서>&z=<항목>)`로 넣고 제목을 선택한다. 행의 링크를 누르면 로컬 문서의 항목이면 그 항목으로
  줌하고, 아니면 브라우저로 연다. `show_backlinks`가 켜지면 줌한 항목을 가리키는 항목을 문서 아래에("This item is referenced in:"),
  가리켜진 행에는 개수 칩(`.o-backlinks`)을 보이고 칩을 누르면 References 대화상자가 열린다. 붙여넣기: 행 입력란이 붙여넣기를
  직접 처리한다(NSH `PasteBehavior::Report`): 여러 줄, HTML, OPML은 `parse_clipboard` → `paste_items`로 항목이 되고(undo 한 번),
  한 줄은 입력란의 글로 남는다. 노트와 제목에는 글 그대로. 앱에서 복사한 항목을 같은 글로 붙이면 구조째 되살린다(웹의 `clip`).
  선택한 항목에 Mod+V는 그 항목을 붙인 항목으로 바꾼다. 내보내기: 메뉴의 "Export…"가 선택한 항목, 줌한 항목 또는 문서를
  Plain text(들여쓰기 4종)/HTML/OPML로, 보이는 것만(`export_visible_only`, 기기에 저장, 검색 중이면 검색 결과만) 보여 준다.
  데스크톱은 Download(저장 대화상자)와 Copy, 휴대폰은 Copy만(NSH 0.5.0에 공유 시트 없음). 데스크톱 상단 바의 Menu 버튼이 이제
  메뉴(휴대폰과 같은 것)를 연다. 새 토큰: `color.match_fg`(웹 `<mark>`의 검정 글자; 웹 `.o-match`도 이 변수를 쓰도록 바꿨고 계산값은
  같다), `space.search_vw`, `space.modal_vw`, `space.modal_vh`, `space.export_out`(웹의 70vw, 92vw, 76vh, 280px 리터럴; 웹
  `styles.css`와 내보내기 출력란(`.export-out`)도 같은 값으로 이 변수를 읽는다).
  테스트: 헤드리스 `tests/search.rs` 13개(거르기·강조·Escape, 캐럿 복귀, 태그 누르기, Flat/전체 결과와 결과 열기, 휴대폰 검색,
  찾아 바꾸기와 undo, `[[` 제안·고르기·링크 누르기, `#` 제안과 Escape, 백링크 개수·References·목록, 여러 줄/한 줄/HTML 붙여넣기,
  내보내기 세 형식·Copy, 휴대폰 선택 항목 내보내기). 구조 스냅샷에 고정 상태 8개(`looks`/`inline`의 search, `looks`의 links와
  backlinks; 데스크톱·휴대폰). `qa:model`이 search 상태를 웹에서 같은 질의로 재서 대조한다(이때 `.o-match` 글자색 차이를 찾아
  맞췄다). 기존 정답은 `[views]`의 들여쓰기만 바뀌었다(문서 영역과 목록 끝에 뷰가 하나씩 늘었다; `[model]`은 같음).
  웹과 다른 점: 링크 후보의 순서는 문서 순서다(웹은 노드 맵의 삽입 순서), 앱 주소가 없어 링크 URL에 origin이 없다(웹은 `#`로 시작하는
  URL도 자기 것으로 연다), 결과 화면의 정렬과 내보내기의 들여쓰기는 `<select>` 대신 버튼, ~~북마크 별표는 그리기만 한다~~(영역 14), 검색어는 웹처럼
  주소(route)에 남지 않는다, ~~문서별 "체크 항목 보기" 설정이 없어 다른 문서의 결과는 체크 항목을 보인다~~(영역 15). 영역 12의 설정 화면에
  이 기능들의 설정 4개를 웹과 같은 이름·라벨·구역으로 더했다: Input의 "Auto-complete tags from all docs"
  (`global_tag_autocomplete`), "Internal links" 구역의 "Show backlinks"(`show_backlinks`), Advanced의 "Show search tip popup"
  (`show_search_tips`, 기본 켬)과 "Export visible items only (default)"(`export_visible_only`; 내보내기 대화상자의 체크도 이 값을 바꾼다).
  `[[`/`#` 목록은 캐럿의 화면 위치(`CaretEvent::page_bounds`) 아래에, 캐럿 위치를 모르면(헤드리스, 캐럿 보고 전) 입력란 아래에 둔다.
- [x] 웹의 나머지 화면 (2026-11-15, 영역 14): 읽기 모드, Back/Forward, 도움말, 튜토리얼, 마인드맵, inbox, 파일 찾기를 웹처럼
  옮겼고, 눌러도 아무 일이 없던 버튼을 모두 기능에 연결했다(`docview.rs`, `help.rs`, `mindmap.rs`, `inbox.rs`, `finder.rs`).
  - 읽기 모드: 휴대폰 상단 바의 자물쇠 버튼과 데스크톱의 View options 메뉴("Lock document (read-only)")가 문서별 설정
    `v_locked`를 웹과 같은 자리(작업 공간 문서의 파일 항목)에 저장한다. 잠긴 문서는 편집 캐럿이 놓이지 않고, 명령과 undo가
    문서를 바꾸지 않으며, 휴대폰 툴바가 숨는다. 불릿으로 접고 펴기, 줌, 선택은 웹처럼 계속 된다.
  - Back/Forward: 문서를 바꾸거나 줌한 모든 변경을 `store::edit`에서 기록하고(웹이 route마다 history를 쌓는 것과 같다, 100개까지),
    데스크톱 상단 바의 Back/Forward와 Alt+←/→가 그 자리로 돌아간다. 돌아가면 아무것도 편집하지 않고(`setZoom`), 사라진 문서는
    건너뛴다. 기존 Back 버튼은 줌 아웃만 했다.
  - 도움말: Mod+/와 메뉴의 "Shortcuts & formatting"이 웹의 도움말 패널(트리거, 서식, 지금 키맵의 명령별 키를 그룹별로)을 연다.
    데스크톱은 문서 오른쪽 340pt 패널, 휴대폰은 화면 전체다. 웹도 첫 실행에 투어를 자동으로 띄우지 않으므로, 메뉴에 Tutorial
    (단계, 점, Skip/Next/Done, 끝내면 `finished_onboarding` 저장)과 "What's new"를 더했다.
  - 마인드맵: View options의 "Layout: Mind map (read-only)"(`v_layout`)가 줌한 항목의 마인드맵을 화면 전체에 띄운다. 상자는 인라인
    서식, 노트, 제목 크기, 색 라벨, 체크 모양을 갖고, 자식은 오른쪽 열에 선으로 잇는다. 상자의 버튼은 지도 안에서만 접고 펴며,
    더블 클릭은 그 항목으로 줌하고(Back으로 돌아올 수 있음), 경로를 누르면 줌 아웃, −/+는 배율(0.3~2), "← Back to document"와
    Escape는 목록으로 돌아간다.
  - Inbox: 설정의 "Your inbox" 구역(목적지 표시와 Change… 찾기 창, "Move to position": `inbox_move_position`), 메뉴와 Mod+Shift+I의
    "Capture to inbox"(여러 줄은 들여쓰기대로 여러 항목, Add 또는 Mod+Return), 메뉴와 사이드바 받은 편지함 버튼의 "Go to inbox".
    목적지가 없으면 웹의 안내 알림을 보인다. 항목 메뉴의 "Set as inbox"는 영역 13의 항목 메뉴가 `inbox::set(doc, id)`를 부르면 된다.
  - 찾기 창(`finder.rs`, 웹 `finder`): 퍼지 순위 50개, ↑/↓, Return, 누르기, Escape. Mod+O와 사이드바 찾기 버튼의 문서·북마크 찾기,
    inbox 고르기, 휴대폰 툴바와 Mod+Shift+M의 "Move item to…"(같은 문서는 옮기고 다른 문서는 복사 후 지움, 웹처럼 목적지 끝에)가 쓴다.
  - 그 밖의 버튼: 데스크톱 문서 오른쪽 위의 doc actions(Bookmark this view, View options), 검색 상자의 "Bookmark this search"
    (검색어를 북마크에 넣고, 그 북마크를 열면 검색이 다시 열린다), 검색 상자 닫기 아이콘(경로가 비어 그려지지 않았다).
  - 고친 버그: 대화상자나 메뉴에서 띄운 알림은 띄운 뷰가 사라지면 타이머가 취소되어 영영 남았다(`toast.rs`가 이제 owner 밖에서
    타이머를 만든다).
  - 새 토큰: `space.capture_vh`(캡처 상자 최소 높이, 웹의 40vh), `type.tour_line`(튜토리얼 줄 높이, 웹의 1.6). 웹 `styles.css`도
    같은 값으로 이 변수를 읽는다.
  - 테스트: 헤드리스 `tests/screens.rs` 10개(휴대폰 자물쇠와 데스크톱 메뉴의 잠금, Back/Forward와 키, 도움말의 사용자 키,
    휴대폰 메뉴의 도움말·튜토리얼·What's new, 마인드맵 접기·더블 클릭 줌·경로·돌아가기, inbox 고르기·캡처·이동, 파일 찾기와
    Mod+O, 검색 북마크, 다른 문서로 Move to). 구조 스냅샷에 고정 상태 6개(`looks`의 help, mindmap, tour; 데스크톱·휴대폰).
    기존 정답은 데스크톱의 `[views]` 끝에 doc actions가, 설정 화면에 "Your inbox" 구역이 더해져서 바뀌었다(`[model]`은 같음).
    `qa:model`은 차이 없음(알려진 차이 91개 그대로).
  - 웹과 다른 점: ~~View options는 레이아웃(List, Mind map)과 잠금만 있다(문서별 체크·노트 보기, Article 레이아웃, 글 방향은 아직
    없다). 웹의 하위 메뉴 대신 한 메뉴에 "✓"로 표시한다.~~(영역 15: 글 방향만 없다) 메뉴에 Version history, Print, Sync status, ~~About~~(영역 15)가 없다. ~~파일 찾기의 보조
    줄은 바로 위 폴더 이름만 보인다(웹은 전체 경로).~~(영역 15) 마인드맵의 배율은 CSS `transform` 대신 크기 값을 곱해서 그린다. ~~Flat search와
    전체 검색의 북마크는 문서의 검색 상자로 연다.~~(영역 15) ~~`move_item_sent_to_position` 설정이 없어 Move to는 늘 목적지 끝에 넣는다~~(영역 13에서 설정을 더함).
- [ ] 버전 기록(웹 `history.ts`)과 공유(`share.ts`): 서버가 필요하다. 아래 서버 동기화와 함께 한다.
- [x] 남은 편집 기능 (2026-11-15, 영역 13): 서버 동기화는 범위 밖.
  항목 메뉴(`itemmenu.rs`, 웹 `itemmenu.ts`): 불릿(과 제목) 오른쪽 클릭, 휴대폰은 길게 누르기. 웹의 순서·라벨(Collapse/Expand,
  Expand to level 1~4, 형제 접기, Zoom in, 노트, Delete, 체크 항목 삭제, 북마크, Sort ▸ 하위 메뉴 11종, 찾아 바꾸기, Indent/Unindent,
  Move to…, Insert template…, Duplicate, 체크박스·자식 체크박스·체크·자식 체크 해제·번호, Get link, 참조, Export…, 색 라벨·제목 견본)과
  같은 명령의 키 표시. 선택 항목 위에서 열면 선택 항목 전체에 적용한다. 메뉴(`menu.rs`)에 키 표시, 하위 메뉴, 자체 행, 창보다 길면 스크롤을 더했다.
  hover 툴바(`item_toolbar_hover`, 설정 Control 구역에 추가): 데스크톱에서 행 위 오른쪽에 웹의 버튼 13개(올리기는 없음).
  키맵: Format 묶음 전부(굵게·기울임·코드·코드 블록·링크·취소선·강조·LaTeX·줄바꿈·제목/색 순환과 단계, 날짜/시각 넣기)와 나머지
  목록·이동 명령(정렬, 펼침 단계, 형제 줌, 부모/형제/처음/끝 이동, 복제, 체크 항목 삭제, 번호, 자식 체크박스 등)을 웹 `Editor.exec`의
  이식(`commands::exec_on`)으로 실행한다. 앱 명령: Move item to…, Insert template…, 파일 찾기(Mod+O)·항목 찾기(Mod+Shift+O)
  (`finder.rs`, 웹 `finder`), 찾아 바꾸기, 링크 복사·Get link·참조, 북마크, 북마크/태그 창 토글. 남은 명령(help, back/forward,
  inbox, version history 등)은 영역 14 몫이다.
  날짜 칩: 누르면 그 날짜로 날짜 선택기가 칩 아래(데스크톱) 열린다. 마우스(데스크톱): Shift+클릭 범위 선택, Mod+클릭 토글, 글자에서
  다른 행으로 끌면 블록 선택, 불릿을 끌어 항목(선택 항목)을 옮기기(웹의 before/after/inside 판정, Mod를 누르고 끌면 복사, 웹의
  드롭 선과 "Move N items" 표시). Move to…(`move_item_sent_to_position`, Advanced에 추가)는 휴대폰에서 항목 메뉴로 쓴다.
  제목(줌 루트)과 그 노트를 제자리에서 편집(Shift+Enter 왕복, Enter는 첫 항목, ↓는 첫 항목, 맨 위에서는 문서 이름도 바뀜).
  상단 메뉴에 "Import OPML / text…". WorkFlowy 키를 켜면 Alt+←/→가 Back/Forward 대신 줌이 되도록 웹(`keymap.ts`, 테스트
  `apps/web/test/editor/keymap.test.ts`)과 네이티브를 함께 고쳤다(사용자가 직접 준 키는 그대로).
  테스트: 헤드리스 `tests/gaps.rs` 14개, 키맵 단위 테스트. 구조 스냅샷에 상태 5개(항목 메뉴·드롭 표시: 데스크톱·휴대폰, hover 툴바:
  데스크톱); `qa:model`이 웹 측정기에서 같은 항목 메뉴를 열어 대조한다(알려진 차이 2개: 웹에만 있는 Share…·Set as inbox와 "Expand to
  level" 행 표기, macOS 고정 장치의 ⌘ 키 표기). 기존 정답은 `[views]`만 바뀌었다(불릿을 감싼 뷰, 날짜 칩·제목의 test id, 메뉴의
  스크롤 뷰, 메뉴 글자 폭; 설정 화면은 항목 2개 추가).
  웹과 다른 점: 메뉴의 Share…(서버)와 Set as inbox(영역 14)가 없고, hover 툴바에 파일 올리기가 없다. ~~메뉴는 웹과 달리 선택 항목에
  속한 불릿에서 열 때만 선택 항목 전체에 적용한다(웹은 다른 항목을 편집 중이면 그 항목에 적용).~~(영역 15에서 웹을 네이티브처럼 고침) ~~끌기 중 자동 스크롤, 사이드바 문서로
  끌어 옮기기, 하위 메뉴의 hover 열기는 없다(누르면 열림).~~(영역 15)
- [x] 앞선 영역의 후속 작업 (2026-11-15, 영역 15): 서버 동기화와 실기기 확인은 범위 밖.
  - 웹 버그 2건(각각 vitest): 같은 폴더 안에서 파일 행을 아래로 끌면 한 칸 더 내려가던 것을 고쳤다(`sidebar.ts`의 `dropIndex`, 북마크
    끌기도 같은 함수를 쓴다; `test/ui/sidebar.test.ts`). 항목 메뉴와 hover 툴바는 이제 연 항목에 적용하고, 그 항목이 선택 안에 있을
    때만 선택 항목 전체에 적용한다(오케스트레이터 결정; `itemmenu.ts`의 `menuTargets`/`itemTargets`, 메뉴의 Export…도 같다;
    `test/ui/itemmenu.test.ts`). 전에는 다른 항목을 편집 중이면 그 항목에 적용했다.
  - 문서별 보기 설정(`docview.rs`): View options가 웹처럼 하위 메뉴(Checked items: Global (설정값)/Show/Hide, Notes: Global/Show/
    1st line/Hide, Layout: List/Article/Mind map, 그리고 잠금)이고, 고른 값에 "✓"를 붙인다. 값은 웹과 같은 자리(작업 공간 문서의 파일
    항목 `v_checked`, `v_notes`, `v_layout`, JSON 문자열)에 두고, 첫 값(Global, List)은 지운다. 열린 문서의 체크 항목 표시는 앱
    root의 effect(`docview::apply`)가 문서·설정·문서 값이 바뀔 때 적용하고, 전체 검색 결과도 문서마다 그 문서의 값을 따른다.
    Article 레이아웃(`.o-article`): 접힌 항목이 아니면 불릿은 행에 포인터가 있을 때만 보이고(이름 있는 group hover), 가이드는
    투명하고 bullet thread가 없으며, 행 위아래에 `space.article`(기존 토큰)을 더한다. 글 방향(Text direction)은 NSH에 오른쪽에서 왼쪽
    레이아웃이 없어 메뉴에서 뺐다.
  - 메뉴(`menu.rs`): 데스크톱에서 하위 메뉴가 있는 항목에 포인터를 올리면 하위 메뉴가 열리고, 다른 항목에 올리면 닫힌다(웹
    `onmouseenter`; 이미 열린 하위 메뉴는 다시 열지 않는다). 누르기로도 열린다.
  - 항목 끌기(`outline.rs`): 목록 위/아래 50pt 안에서는 50ms마다 10~30pt씩 스크롤하고(웹 `autoscroll`의 값), 스크롤한 뒤 행을 다시
    재서 놓일 자리를 다시 찾는다. 놓으면 멈춘다. 문서 열 왼쪽(사이드바)으로 끌면 Files 탭의 문서 행(점선 강조 테두리
    `.drop-target`)이나 빈 곳(창 테두리)이 놓일 곳이 되고, 놓으면 웹 `dropItems`처럼 문서로 옮기거나(Mod를 누르고 있으면 복사,
    "Moved to …"/"Copied to …"), 빈 곳이면 항목마다 같은 폴더에 새 문서(이름은 항목 글, 내용은 그 자식)를 만든다(`finder::drop_items`).
  - 파일 찾기(Mod+O)의 보조 줄은 웹처럼 폴더 전체 경로("A / B")이고 보관한 문서는 "(archived)"를 붙인다(`workspace::path_of`).
  - 검색 북마크: 결과 화면을 연 채 북마크하면 웹처럼 Flat search는 `flat`, 전체 검색은 `global`과 "Search: …" 이름, 정렬은 `sort`로
    저장하고(`workspace::add`), 열면 그 결과 화면을 그 정렬로 다시 연다.
  - 6번(작은 누락 기능): 메뉴에 About(웹 `aboutDialog`, 동기화는 "local only")과 "Import from WorkFlowy…"(안내 대화상자와 OPML
    가져오기)를 더했다. 남긴 것과 이유: 글 방향(NSH에 RTL 레이아웃 없음), Version history·Share·Sync status·파일 올리기(서버),
    Print(NSH에 인쇄 없음), 휴대폰 내보내기 공유(NSH 0.5.0에 공유 시트 없음), 서랍 닫기 애니메이션(닫을 때 뷰를 바로 내림), 사이드바
    끌기 중 따라오는 행 그림, 반복 날짜 툴팁(NSH에 툴팁 없음), 항목 선택 중 날짜 팝오버 위치.
  - 테스트: 헤드리스 `tests/polish.rs` 9개(문서별 체크 항목·레이아웃과 저장·다른 문서와 독립, 문서별 노트, 하위 메뉴 hover 열기·닫기와
    항목 메뉴의 Sort, 파일 찾기 경로·보관 표시, Flat·전체 검색 북마크의 저장과 다시 열기, 끌기 자동 스크롤과 놓은 뒤 멈춤, 사이드바
    문서로 옮기기, 빈 곳에서 새 문서, About·WorkFlowy). `tests/screens.rs`의 마인드맵 테스트는 Layout 하위 메뉴를 거치도록 바꿨다.
    구조 스냅샷에 고정 상태 2개(`looks-{desktop,phone}-article`)를 더했고 웹 측정기(`test/model/main.ts`의 'article')와 `qa:model`이
    대조한다(차이 0, 새 알려진 차이 없음). 기존 정답은 `[views]`만 바뀌었다(파일 목록을 감싸는 뷰 하나; `[model]`은 같음).
  - 웹과 다른 점: 하위 메뉴에 글 방향이 없다. 사이드바 놓기 판정은 끌기 시작 때 잰 행 위치로 하므로 끄는 동안 사이드바를 스크롤하면
    어긋날 수 있다(웹은 `elementFromPoint`).
- [ ] 서버 동기화 연결(HTTP/WS 클라이언트, 로그인), 문서 목록, 줌.
- [ ] 알려진 제약: Rust 문자열은 짝이 맞지 않는 UTF-16 서로게이트를 담을 수 없다. 웹이 이런 문자열을 보내면 역직렬화가 실패한다.
- 명령:
  ```sh
  for g in gen gen-commands gen-inline gen-search gen-refs gen-paste gen-export gen-calendar; do node native/crates/outliner-core/fixtures/$g.ts; done
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
