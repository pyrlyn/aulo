package aulo.grpc

import aulo.grpc.nghttp2.nghttp2_version
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.pointed
import kotlinx.cinterop.toKString

/** The version of the nghttp2 library linked into this binary, for example `1.68.1`. */
@OptIn(ExperimentalForeignApi::class)
fun nghttp2Version(): String = checkNotNull(nghttp2_version(0)?.pointed?.version_str) { "nghttp2 reports no version" }.toKString()
