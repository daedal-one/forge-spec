import type { Session } from './session'

const REAPER_INTERVAL_SECONDS = 60

export class SessionReaper {
  removeExpired(
    sessions: Session[],
    now: number,
    currentCredentialsVersion: number,
  ): Session[] {
    return sessions.filter((session) => {
      const tooOld = now - session.issuedAt >= 30 * 24 * 60 * 60
      const tooIdle = now - session.lastSeenAt >= 14 * 24 * 60 * 60
      const staleCredentials = session.credentialsVersion < currentCredentialsVersion
      return !(tooOld || tooIdle || staleCredentials)
    })
  }
}

export const sessionReaperInterval = REAPER_INTERVAL_SECONDS
