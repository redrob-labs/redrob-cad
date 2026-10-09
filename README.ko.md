# 레드롭 캐드

AI 에이전트가 DWG·DXF 도면을 대신 그리고, 고치고, 검토하는 에이전틱 2D 제도 앱.

English: [README.md](./README.md)

Rust로 작성된 [CADCraft](https://github.com/storytold/cadcraft)의 하드 포크다. macOS, Windows, Linux,
FreeBSD에서 네이티브로, 브라우저에서는 WebAssembly로 돈다. 에이전트는 MCP로 앱을 조작한다
(`crates/mcp`, [docs/mcp.md](./docs/mcp.md) 참고).

## 상태

초기 단계. 트리는 CADCraft `v0.3.0`에 포크 파일을 더한 것이다. 다음 순서로 PR 하나씩 진행한다.

1. 리브랜딩(이름, 앱 ID, 아이콘, 정보 문구; 상류 상표 제거).
2. 내장 에이전트로 Redrob Code 연결(MCP로 앱과 대화).
3. Connect device로 레드롭 워크스페이스 로그인.

## 빌드와 테스트

요구 사항: Rust stable(1.90 이상). Linux에서는
[`.github/workflows/ci.yml`](./.github/workflows/ci.yml)에 나열된 X11/Wayland 개발 라이브러리.

```sh
git clone https://github.com/redrob-labs/redrob-cad.git
cd redrob-cad
cargo run -p cadcraft --release   # 데스크톱 앱
cargo xtask ci                    # CI가 돌리는 게이트
```

## 문서

- 포크 지점, 상류 흡수 방법, 이 포크가 소유하지 않는 것: [UPSTREAM.ko.md](./UPSTREAM.ko.md)
- 귀속: [NOTICE](./NOTICE)
- 기여 규약: [redrob-labs/.github](https://github.com/redrob-labs/.github)의 CONTRIBUTING.ko.md, FORKS.ko.md

## 라이선스

상류와 같이 MIT OR Apache-2.0 중 선택. [LICENSE-MIT](./LICENSE-MIT),
[LICENSE-APACHE](./LICENSE-APACHE), [NOTICE](./NOTICE) 참고.
