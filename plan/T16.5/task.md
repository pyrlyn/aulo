# T16.5. Windows named pipe admits any writing client as `LocalOwner`

`crates/aulo-server/src/auth.rs:161-174` inserts `Caller::LocalOwner` for any pipe client, relying on the default DACL — weaker than the Unix `SO_PEERCRED` uid check (`auth.rs:136-153`) and than the module's "fails closed" claim. Done means: `GetNamedPipeClientProcessId` (or an equivalent owner check) is verified before `LocalOwner` is granted.
