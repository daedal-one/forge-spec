export interface Session {
  issuedAt: number
  lastSeenAt: number
  credentialsVersion: number
}

export class SessionStore {
  expire(
    session: Session,
    now: number,
    currentCredentialsVersion: number,
  ): boolean {
    const wallClockExpired = now - session.issuedAt >= 30 * 24 * 60 * 60
    const idleExpired = now - session.lastSeenAt >= 14 * 24 * 60 * 60
    const credentialsRotated = session.credentialsVersion < currentCredentialsVersion

    return wallClockExpired || idleExpired || credentialsRotated
  }
}
