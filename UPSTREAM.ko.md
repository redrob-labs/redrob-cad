# 상류: storytold/cadcraft

이 저장소는 [storytold/cadcraft](https://github.com/storytold/cadcraft)의 **하드 포크**다. 상류의 릴리스
태그 하나까지 상류 히스토리 전체를 담는다. 패치 스택도 미러도 아니다.

English: [UPSTREAM.md](./UPSTREAM.md)

## 포크 지점

| | |
|---|---|
| 상류 태그 | `v0.3.0` |
| 상류 커밋 | `59631c8` — 2026-10-08, `Bump version to 0.3.0 (from 0.2.0) for release` |
| 상류 기본 브랜치 | `main` |
| 우리 장기 브랜치 | `develop`(기본), `main`(릴리스) |

기계가 읽는 기록은 [`upstream-base.json`](./upstream-base.json)이다. 히스토리를 공유하므로
`git merge-base develop upstream/main`이 실제로 존재하고, 상류 흡수는 재적용이 아닌 일반 3방향 병합이다.

## 이전 기반

2026-10-09까지 이 저장소는 FreeCAD 위의 패치 묶음이었다. 그 작업은 지우지 않고 보관한다.

- 브랜치 `archive/freecad`와 태그 `freecad-final`은 마지막 FreeCAD 기반 `develop`인 `a9a11cd`를 가리킨다.
- 브랜치 `archive/freecad-stage3-wip`에는 전환 시점에 진행 중이던 미완성 작업이 있다.

## 이 포크가 소유하지 않는 것

상류 `NOTICE`에 따르면 `docs/brand/`의 ArtCraft 이름·워드마크·로고는 ArtCraft Team의 상표이며 오픈소스가
아니고 코드 라이선스의 대상도 아니다. `upstream-base.json`의 `proprietaryPrefixes`에 이 경로가 있다.
리브랜딩에서 제거했다. 상류 흡수로 다시 들어오거나 상류 제품명·링크가 사용자에게 보이는 곳에 나타나면
`cargo xtask brand`가 실패한다.

## 상류 흡수

상류 **태그**를 따른다. 흡수 브랜치는 `sync/upstream-<tag>`이며 `develop`으로 PR을 연다. **스쿼시가 아닌
병합 커밋으로 병합한다.** 스쿼시는 병합 기준점을 없애서 다음 흡수가 이미 가져온 작업을 다시 재생한다.

```sh
git remote add upstream https://github.com/storytold/cadcraft.git
git fetch upstream --tags
git checkout -b sync/upstream-vX.Y.Z develop
git merge vX.Y.Z          # 충돌은 손으로 해소한다. 우리가 이름을 바꾼 곳은 우리 것을, 그 주변은 상류 것을 받는다
cargo xtask ci
```

같은 PR에서 `upstream-base.json`의 `base`, `baseTag`, `baseDate`, `baseSubject`, `lastSyncedUpstream`을
갱신한다.

## 귀속

상류 저작권 고지는 `LICENSE-MIT`와 `NOTICE`에 그대로 두고, 우리 고지는 그 아래에 덧붙인다. 대신 쓰지 않는다.
코드는 상류와 같이 MIT OR Apache-2.0으로 제공된다.
