package aulo.server

import aulo.secrets.SecretStore
import aulo.store.Store

/** The first token's name, in the secret store and in `api_tokens`. */
const val FIRST_TOKEN = "aulod-token"

/** The scope of the first token: every method (spec §11). */
const val ADMIN_SCOPE = "admin"

/**
 * Makes the admin token on first run, when [store] has none: the value goes to [secrets], only
 * its hash to [store]. Returns the hashes the server accepts.
 */
fun ensureFirstToken(
    store: Store,
    secrets: SecretStore,
): List<String> {
    val known = store.listApiTokens()
    if (known.isNotEmpty()) return known.map { it.hash }
    val token = BearerTokens.generate()
    // The secret first: if recording the hash then fails, the next run makes a new token and
    // overwrites this one, where the other order could leave a hash whose token nobody has.
    secrets.save(FIRST_TOKEN, token)
    return listOf(store.putApiToken(FIRST_TOKEN, ADMIN_SCOPE, BearerTokens.hash(token)).hash)
}
