package aulo.grpc

import aulo.grpc.nghttp2.NGHTTP2_VERSION
import aulo.grpc.nghttp2.nghttp2_session_callbacks_del
import aulo.grpc.nghttp2.nghttp2_session_callbacks_new
import aulo.grpc.nghttp2.nghttp2_session_del
import aulo.grpc.nghttp2.nghttp2_session_mem_send2
import aulo.grpc.nghttp2.nghttp2_session_server_new
import aulo.grpc.nghttp2.nghttp2_submit_settings
import cnames.structs.nghttp2_session
import cnames.structs.nghttp2_session_callbacks
import kotlinx.cinterop.ByteVar
import kotlinx.cinterop.ExperimentalForeignApi
import kotlinx.cinterop.UByteVar
import kotlinx.cinterop.allocPointerTo
import kotlinx.cinterop.memScoped
import kotlinx.cinterop.ptr
import kotlinx.cinterop.readBytes
import kotlinx.cinterop.reinterpret
import kotlinx.cinterop.value
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull

@OptIn(ExperimentalForeignApi::class)
class Nghttp2Test {
    @Test
    fun the_linked_library_is_the_one_the_binding_was_made_from() {
        assertEquals(NGHTTP2_VERSION, nghttp2Version())
    }

    @Test
    fun a_server_session_opens_with_a_settings_frame() {
        memScoped {
            val callbacks = allocPointerTo<nghttp2_session_callbacks>()
            assertEquals(0, nghttp2_session_callbacks_new(callbacks.ptr))
            val session = allocPointerTo<nghttp2_session>()
            assertEquals(0, nghttp2_session_server_new(session.ptr, callbacks.value, null))
            try {
                assertEquals(0, nghttp2_submit_settings(session.value, 0u, null, 0u))
                val frame = allocPointerTo<UByteVar>()
                val length = nghttp2_session_mem_send2(session.value, frame.ptr)
                // A frame header: 24 bits of payload length (no settings: 0), the type (4 is
                // SETTINGS), the flags, and the stream id (0, the connection itself).
                assertEquals(9, length.toInt())
                val header = assertNotNull(frame.value).reinterpret<ByteVar>().readBytes(9)
                assertContentEquals(byteArrayOf(0, 0, 0, 4, 0, 0, 0, 0, 0), header)
            } finally {
                nghttp2_session_del(session.value)
                nghttp2_session_callbacks_del(callbacks.value)
            }
        }
    }
}
