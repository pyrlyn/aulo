# T16.5. Windows named pipe admits any writing client as `LocalOwner`

`modules/aulo-server/src/auth.kt:161-174` inserts `Caller::LocalOwner` for any pipe client, relying on the default DACL — weaker than the Unix `SO_PEERCRED` uid check (the Unix SO_PEERCRED uid check) and than the module's "fails closed" claim. Done means: `GetNamedPipeClientProcessId` (or an equivalent owner check) is verified before `LocalOwner` is granted.
