# CLAUDE.md

seol-gitops 클러스터 스모크 테스트 서버 (Rust, tokio, axum). 문서와 커밋 메시지는 한국어

## 규칙

- 커밋과 푸시는 사용자가 검토 후 직접 수행, 파일 준비와 제안 커밋 메시지까지만
- 커밋 메시지는 `type(scope): 설명` 한 줄 한국어 (예: `feat(server): /healthz 추가`)
- 시크릿은 GitHub Actions Secrets 에만, 레포 파일에 저장 금지
- 의존성과 이미지 버전은 명시적으로 고정, 갱신은 Dependabot PR 로

## 검증

```bash
cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test
docker build -t smoke .
```
