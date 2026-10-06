//! Grants and the audit log. The store only persists: hashes are computed and the
//! chain is verified by `aulo-policy`, so a caller supplies `prev_hash` and `hash`.

use diesel::prelude::*;

use crate::models::{AuditRow, Grant, NewAuditRow};
use crate::schema::{audit, grants};
use crate::{Store, StoreError, now_ms};

impl Store {
    /// Records a grant. A repeated subject and scope adds a row rather than replacing
    /// one, so the history of decisions stays listable.
    pub fn put_grant(
        &mut self,
        subject: &str,
        scope: &str,
        decision: &str,
        expires_at: Option<i64>,
    ) -> Result<Grant, StoreError> {
        let grant = Grant {
            id: self.next_id(),
            subject: subject.to_owned(),
            scope: scope.to_owned(),
            decision: decision.to_owned(),
            expires_at,
            created_at: now_ms(),
        };
        diesel::insert_into(grants::table)
            .values(&grant)
            .execute(&mut self.conn)?;
        Ok(grant)
    }

    /// Every grant, expired ones included, oldest first.
    pub fn list_grants(&mut self) -> Result<Vec<Grant>, StoreError> {
        Ok(grants::table
            .order(grants::id.asc())
            .select(Grant::as_select())
            .load(&mut self.conn)?)
    }

    /// Grants of `subject` that have not expired. The filter lives in the query so an
    /// expired grant can never be returned by a caller forgetting to check.
    pub fn list_active_grants(&mut self, subject: &str) -> Result<Vec<Grant>, StoreError> {
        Ok(grants::table
            .filter(grants::subject.eq(subject))
            .filter(
                grants::expires_at
                    .is_null()
                    .or(grants::expires_at.gt(now_ms())),
            )
            .order(grants::id.asc())
            .select(Grant::as_select())
            .load(&mut self.conn)?)
    }

    /// Returns `false` when no grant has that id.
    pub fn revoke_grant(&mut self, id: &str) -> Result<bool, StoreError> {
        let n = diesel::delete(grants::table.find(id)).execute(&mut self.conn)?;
        Ok(n > 0)
    }

    /// Appends one audit row; `seq` is assigned by the database. There is deliberately
    /// no update or delete: the log is append-only.
    pub fn append_audit(
        &mut self,
        prev_hash: &str,
        hash: &str,
        kind: &str,
        payload: &str,
    ) -> Result<AuditRow, StoreError> {
        let row = NewAuditRow {
            prev_hash,
            hash,
            kind,
            payload,
            created_at: now_ms(),
        };
        Ok(diesel::insert_into(audit::table)
            .values(&row)
            .returning(AuditRow::as_returning())
            .get_result(&mut self.conn)?)
    }

    /// The chain head, which the caller needs to compute the next `prev_hash`.
    pub fn last_audit(&mut self) -> Result<Option<AuditRow>, StoreError> {
        Ok(audit::table
            .order(audit::seq.desc())
            .select(AuditRow::as_select())
            .first(&mut self.conn)
            .optional()?)
    }

    /// Rows with `seq` greater than `after`, in order (`0` starts from the beginning).
    pub fn list_audit_after(
        &mut self,
        after: i64,
        limit: i64,
    ) -> Result<Vec<AuditRow>, StoreError> {
        Ok(audit::table
            .filter(audit::seq.gt(after))
            .order(audit::seq.asc())
            .limit(limit)
            .select(AuditRow::as_select())
            .load(&mut self.conn)?)
    }
}
