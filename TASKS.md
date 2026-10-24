# TASKS — 작업 현황과 남은 일

Dynalist를 클린룸으로 재구현하는 셀프호스팅 아웃라이너. 이 문서만 읽고 이어서 작업할 수 있게 정리했다.
(최종 갱신: 2026-10-24, 커밋 `5c0598e` 기준)

- 레포: https://github.com/jadewisemann/mynalist (public), 로컬 `/home/ubuntu/code/outliner`, 브랜치 `main`
- 구조:
  - `packages/core`: 트리/op 엔진, undo, sync, 에디터 명령, 인라인 문법, 날짜, 검색, 붙여넣기/내보내기. 런타임 의존성 없음.
  - `apps/server`: Node 24, node:sqlite, ws.
  - `apps/web`: Vite, vanilla TS.
  - `apps/android`: Capacitor 8.
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
6. **Android:** Capacitor 8 셸. debug APK 빌드가 되고, 서버 URL은 설정 화면에서 입력한다. → `apps/android/README.md`
7. **PWA:** manifest, 아이콘, service worker. systemd 배포 파일.
8. **반복 날짜:**
   - 반복 날짜 체크 처리(new_instance / update).
   - 에디터 내 팝업: 태그 자동완성, `[[` 항목 링크, 날짜 선택기(반복 포함).
9. **배포 편의:**
   - 루트 `npm start`: web 빌드가 없으면 빌드한 뒤 서빙한다.
   - `SERVER_PORT` 지원.
   - `deploy/install.sh`: 127.0.0.1에만 바인딩하고, node 경로를 고정한다.

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
  | editing | 시나리오 343개 | 전부 일치 |
  | fuzz | 랜덤 키 400개 × 10스텝, 매 스텝 비교 | 399개 일치, 1개 건너뜀 |
  | render | 인라인 문법 117개 | 117/117 |
  | layout | 측정값 129개 | 129/129 |

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
- **이 규칙들의 단위 테스트:** `packages/core/test/parity.test.ts`. 현재 core/server 테스트 166개가 통과한다.

## 남은 일

### 1. 공개 배포 마무리 (사용자 작업이 필요)
- [ ] 사용자가 `sudo tailscale funnel --bg 8080`을 실행하고, 링크가 뜨면 HTTPS/Funnel을 승인한다.
- [ ] 공개 URL(`https://<machine>.<tailnet>.ts.net`)로 접속해 본다:
  - 로그인
  - 설정 → Sync server에 같은 URL로 연결
  - 휴대폰에서 PWA 설치
  - 두 기기 간 동기화
- [ ] 휴대폰 실사용 점검: 모바일 툴바, 가상 키보드, 팝업 위치.
- [ ] DB 백업: 지금은 없다. systemd timer로 `outliner.db`를 주기적으로 복사하는 정도면 된다. sqlite `.backup` 또는 `VACUUM INTO`.
- [ ] 공개 전에 서버 보안을 한 번 더 확인한다: 로그인 시도 제한(현재 있음), CORS 기본값 `*`, 업로드 크기 제한.

### 2. QA에서 남은 차이와 한계
- [ ] **코드 블록:** 문법 강조(highlight.js 계열)와 Copy 버튼을 구현한다. 현재 render 비교에서는 무시하고 있다.
- [ ] **비교할 수 없는 것:**
  - HTML 붙여넣기: 데모가 합성 이벤트로 넣은 HTML을 무시한다. 우리 쪽 e2e로 따로 검증할 것.
  - 여러 문서, 동기화, 설정, 색 라벨(Ctrl+Shift+L): 데모에 없거나 동작하지 않는다.
- [ ] **QA를 더 넓힐 곳:**
  - 검색(쿼리 문법, 하이라이트)
  - 컨텍스트 메뉴와 단축키 목록이 Dynalist와 같은지
  - 모바일 뷰포트에서의 비교
- [ ] **녹화 다시 하기:** Dynalist가 바뀌었는지 가끔 `npm run qa:live`로 확인하고, 필요하면 `npm run qa:record`.
- [ ] **CI 관리:**
  - ubuntu-latest가 Ubuntu 26으로 바뀌는 것(2026-10-19부터)에 대비한다.
  - `actions/upload-artifact@v4`가 Node 20 기반이라 경고가 나면 올린다.

### 3. 스펙 대비 미구현/부분 구현 기능
(아래 목록은 `docs/0x` 스펙과 코드를 대조해서 뽑았다. 다음 두 가지는 갭이 아니다.)
- 글자 크기와 간격 값이 스펙과 다른 것: QA에서 Dynalist를 측정해 일부러 맞춘 값이다.
- 분리된 노드 purge가 스펙의 48시간이 아니라 30일인 것: 의도한 값이다.

**P1: 개인용으로 매일 쓰려면 필요한 것**
- [ ] **Inbox:**
  - 빠른 입력(Ctrl+Shift+I)과 Go to inbox를 구현한다. 지금은 커맨드만 있고 false를 반환한다. "Set as inbox"는 설정값만 저장한다.
  - inbox 위치와 넣을 위치 설정.
- [ ] **페이지를 떠날 때:** 저장 안 된 변경이 있으면 경고한다. 지금 `beforeunload`는 `flushSaves()`만 부른다.
- [ ] **백업:** 앱에서 백업 다운로드(zip)와 주기 백업. 서버 쪽 백업은 1번 참고.
- [ ] **버전 기록:**
  - 서버에 history 엔드포인트를 만들고 버전을 묶어 보여주는 모달을 만든다.
  - op 로그는 이미 있으니 이를 활용한다.
- [ ] **검색:**
  - Search & replace 모달: `search-replace` 커맨드는 keymap에 있지만 처리하는 코드가 없다.
  - Flat search와 Global search는 스펙상 결과 화면 방식이다. 지금은 finder 팝업이다:
    - 정렬
    - "Search results for…" 헤더
    - Esc로 돌아가기
    - 문서별 카드
    - 숨겨진 결과 수 표시
  - `is:completed` 필터를 쓸 때 체크된 항목을 강제로 보이게 한다. `view.ts`의 `visibleChildren` 수정.
  - 검색에 걸린 note는 숨김 설정이어도 보이게 한다.
- [ ] **정렬 메뉴:** 지금은 제목 A→Z/Z→A만 있다. 날짜, 체크된 항목 먼저/나중, 수정·생성 시각, 역순을 추가한다.
- [ ] **Backlinks:**
  - "This item is referenced in:" 섹션.
  - 참조 수 배지.
  - Show all references.
- [ ] **기존 기능에서 빠진 부분:**
  - `[[` 링크의 제목 자동 표시: core에 `linkTitle`이 있지만 `renderOpts`가 넘기지 않는다.
  - Unindent in place: `EditorPrefs.unindentInPlace`는 있지만 아무 데서도 쓰지 않는다.
  - 문서별로 마지막 줌 위치 기억.
  - bullet을 더블클릭하면 자식 전체 펼치기/접기.
  - 기한 지난 날짜 강조가 체크된 항목에도 붙는다. 체크된 항목은 제외해야 한다.
- [ ] **클립보드:**
  - `text/opml`을 복사·붙여넣기 모두 지원한다.
  - 번호 목록을 복사할 때 `N.` 접두사를 붙인다.
- [ ] **내보내기:**
  - 여러 항목을 선택했을 때 첫 항목만 내보내진다.
  - "보이는 항목만" 옵션이 체크/노트 상태를 반영하지 않는다.
  - HTML 내보내기는 완전한 문서 형태로 만든다.
  - OPML 헤더에 owner 정보를 넣는다.
  - 인쇄 스타일: 10pt, 흰 배경, `print-color-adjust`.

**P2: Dynalist다운 UI/UX**
- [ ] **상단 메인 메뉴:** 지금은 도움말만 연다. 백업 다운로드, Import from WorkFlowy, About이 있어야 한다.
- [ ] **Settings 화면:**
  - 탭으로 나눈다: Preferences, Keymap, advanced.
  - Keymap 편집: `Keymap`은 `overrides`를 받을 수 있지만 `shortcut_override`를 읽거나 쓰는 곳이 없다.
  - 빠진 설정들:
    - 현재 항목 강조: 지금은 항상 켜져 있다.
    - 맞춤법 검사: `editor.ts`에 꺼짐으로 고정돼 있다.
    - 단어 수 표시, 글꼴/동아시아 글꼴/난독증 글꼴
    - RTL, 태그 배경, 문서 테두리, 첫 항목 자동 포커스, 검색 팁
    - 날짜 표시 형식: core의 `dateFormat`이 연결돼 있지 않다.
    - 시간대 기록, 줄바꿈 정리, Custom CSS
    - 항목 이동 위치 UI, `export_visible_only` 저장
- [ ] **문서별 보기 옵션:**
  - 체크된 항목 표시, 노트 표시, 레이아웃(Article), 텍스트 방향.
  - 문서별 사용자 데이터를 저장하는 곳이 아직 없다.
- [ ] **사이드바:**
  - 파일 패널 툴바: 파일 찾기, inbox, 접기.
  - 문서 메뉴에 Make a copy, Version history, Clean up newlines를 추가한다.
  - 항목을 문서로 끌어서 새 문서 만들기/옮기기, 문서를 항목으로 끌어서 넣기.
  - 보관된 문서는 미리 불러오거나 동기화하지 않는다. 지금은 `sync.ts`, 태그 집계, 전역 검색이 모두 불러온다.
- [ ] **북마크:**
  - 포커스 노드, flat 여부, 정렬도 저장한다.
  - 더블클릭으로 이름 바꾸기, 삭제 확인.
  - 제목과 검색창 옆에 북마크 아이콘.
  - 북마크 패널 단축키가 열기만 하고 닫지 못한다.
- [ ] **태그 패널:** 정렬 토글, 섹션 접기, 태그 이름 바꾸기/삭제(core의 `replaceTag` 활용).
- [ ] **항목 hover:**
  - 줌 아이콘.
  - bullet 툴팁에 생성/수정 시각.
  - 이미지 미리보기.
  - 데스크톱용 항목 툴바.
  - 접기/펼치기 애니메이션.
- [ ] **모바일 툴바:**
  - 버튼 추가: 삭제, 줌, 체크박스, 색, 헤딩, 복제, 날짜 등.
  - 사용자가 버튼 순서를 바꿀 수 있게.
- [ ] **도움말:**
  - 모달이 아니라 옆에서 열리는 패널로.
  - 트리거 표(`#`, `@`, `!`, `[[`).
  - 단축키가 없는 커맨드도 목록에 포함.
- [ ] **URL과 링크:**
  - URL에 포커스 노드, flat, 정렬을 담는다.
  - 브레드크럼을 Ctrl/Cmd+클릭하면 새 탭에서 연다.
  - Get link를 누르면 URL 모달을 보여준다.
- [ ] **상태 표시:**
  - 동기화 상태 모달에 문서별 오류를 보여준다.
  - "Cannot open document"와 "Failed to load + Retry" 빈 화면.
  - 단어 수 상태바.
- [ ] **누락된 단축키:**
  - 부모/이전 형제/다음 형제로 이동, 항목 처음/끝으로 이동.
  - 이 문서에서만 체크/노트 토글.
  - 맞춤법 검사 토글.
  - 템플릿 삽입.
  - Mac Ctrl+P/N.
- [ ] **테마 추가:** Sci-Fi, Sakura, Dark gold, Christmas.
- [ ] **LaTeX:** 지금은 원문을 보여준다. 수식 렌더링을 넣을지 결정한다(KaTeX 등).

**P3: 범위 밖이거나 나중에 (단일 사용자 셀프호스팅이라 우선순위 낮음)**
- [ ] 파일 업로드: 클라이언트와 서버 모두, 업로드 관리자 포함.
- [ ] 공유, 공개 뷰어, 협업자, 권한(namespace, linkfile). 지금 서버는 단일 사용자다.
- [ ] Mind map 뷰, Article 레이아웃, RTL.
- [ ] Google Calendar 연동, 이메일로 inbox에 보내기, Dropbox/Drive 자동 백업.
- [ ] 온보딩 튜토리얼(지금은 Welcome 문서), What's new, 읽기 전용 잠금 모드.

### 4. Android
- [ ] 실제 기기에서 debug APK를 테스트한다: 서버 URL 입력, 동기화, 키보드.
- [ ] 필요하면 release 서명 빌드를 만든다. 키스토어는 레포 밖에 둔다.

## 자주 쓰는 명령

```sh
npm test && npm run typecheck                    # core/server 테스트
npm run qa:serve && npm run qa                   # Dynalist 비교(오프라인 녹화본)
npm run qa:live                                  # 라이브 Dynalist와 나란히
node qa/run.mjs editing --only=undo/ --verbose   # 일부만, 차이를 바로 출력
node qa/show.mjs fuzz fuzz/123                   # 차이를 나란히 보기 (단계별)
FUZZ_FROM=5001 FUZZ_N=200 node qa/run.mjs fuzz   # 새 시드로 탐색 (라이브)
apps/web/e2e/snapshot.sh && node apps/web/e2e/keys.mjs   # 우리 쪽 e2e (popups/drag/mobile/sync도 있음)
npm run build -w @outliner/web && systemctl --user restart outliner   # 배포 갱신
```

- **QA 리포트:** `qa/out/<suite>/report.html`. gitignore 대상이고, CI에서는 artifact로 올라간다.
- **알아둘 함정:**
  - `pkill -f`의 패턴이 자기 명령줄에도 들어 있으면 자기 셸까지 죽인다.
  - Dynalist는 키 입력 간격이 60ms보다 짧으면 순서가 꼬인다. 부하가 걸리면 undo 묶음 타이밍이 흔들리는데, 러너가 단독 재실행으로 걸러준다.
