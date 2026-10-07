---
id: TASK:core/untracked-symlink-identity
type: task
status: accepted
summary: Match the provider's non-dereferencing untracked symbolic-link identity.
owners: [carlo]
progress: done
addresses:
  - REQ:core/intellect-provider#c-protocol
  - REQ:core/intellect-provider#c-lifecycle
labels: [adherence, workspace, regression]
assignee: carlo
---

# Untracked symbolic-link identity

## Acceptance

The client and Forge Intellect provider derive the same workspace identity from untracked symbolic-link paths and link-target bytes without following links. Directory and dangling targets remain readable as links, external target content does not affect identity, retargeting changes identity, and regular-file identities retain their existing encoding. Unsupported special files fail explicitly. Regression tests cover these cases and a real DeepSeek Harness adherence query.

## Verification

The provider-client tests and complete Forge Spec suite pass. The installed local CLI completes DeepSeek Harness implementation status with its existing untracked virtual-environment symlink and the corrected Forge Intellect provider. Evidence states remain unverified or unknown where attestations or source boundaries are absent.
