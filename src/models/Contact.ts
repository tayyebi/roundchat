/** A contact sourced from CardDAV. */
export interface Contact {
  /** vCard UID. */
  id: string;
  /** Full display name. */
  displayName: string;
  /** Primary email address. */
  email: string;
  /** Optional phone number. */
  phone?: string;
  /** Optional avatar URL (photo data URI or HTTP URL). */
  avatarUrl?: string;
}
