package aulo.app.daemon

/**
 * Prepares for the requests to stop aulod: SIGINT and SIGTERM on Unix; Ctrl+C, Ctrl+Break and
 * closing the console on Windows. Call it first thing in `main`, before any other thread starts,
 * so every thread inherits what it sets up.
 */
expect fun installShutdownSignals()

/** Waits for the first stop request and returns its name, for example `SIGTERM`. */
expect suspend fun awaitShutdownSignal(): String
