# TASKS — 작업 현황과 남은 일

Dynalist를 클린룸으로 재구현하는 셀프호스팅 아웃라이너. 이 문서만 읽고 이어서 작업할 수 있게 정리했다.
(최종 갱신: 2026-10-24)

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
    - **Android:** release 서명 설정. 키스토어는 레포 밖에 두고 `scripts/make-keystore.sh`로 만든다.

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

### 4. Android
- [ ] 실제 기기에서 debug APK를 테스트한다 (사용자). 테스트 항목:
  - 서버 URL 입력
  - 동기화
  - 키보드
  - 파일 업로드
- [x] release 서명 빌드 준비:
  - `apps/android/scripts/make-keystore.sh`로 `~/.config/outliner`에 키스토어와 속성 파일을 만든다.
  - 빌드는 `npm run build:release -w @outliner/android`.
- [ ] 키스토어를 만들어 release APK를 빌드하고, 키스토어를 안전한 곳에 백업한다 (사용자 판단).

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
