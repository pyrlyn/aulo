package aulo.server

import aulo.grpc.StatusCode
import aulo.server.generated.GrpcSchema
import io.grpc.reflection.v1.ErrorResponse
import io.grpc.reflection.v1.ExtensionNumberResponse
import io.grpc.reflection.v1.FileDescriptorResponse
import io.grpc.reflection.v1.ListServiceResponse
import io.grpc.reflection.v1.ServerReflectionRequest
import io.grpc.reflection.v1.ServerReflectionResponse
import io.grpc.reflection.v1.ServerReflectionServer
import io.grpc.reflection.v1.ServiceResponse
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.channels.SendChannel
import okio.ByteString
import okio.ByteString.Companion.decodeBase64

/**
 * `grpc.reflection.v1.ServerReflection` over the descriptors build-logic writes at build time
 * (Kotlin/Native has no runtime reflection). Lists only [services], the ones the server serves.
 */
internal class ReflectionService(
    services: Collection<String>,
) : ServerReflectionServer {
    private val services = services.sorted()

    // Decoded once, on first use; calls may run on several threads, so no lazily filled map.
    private val descriptors: Map<String, ByteString> by lazy {
        GrpcSchema.descriptors.mapValues { (file, base64) ->
            base64.decodeBase64() ?: error("the generated descriptor of $file is not base64")
        }
    }

    override suspend fun ServerReflectionInfo(
        request: ReceiveChannel<ServerReflectionRequest>,
        response: SendChannel<ServerReflectionResponse>,
    ) {
        for (message in request) response.send(answer(message))
    }

    fun answer(request: ServerReflectionRequest): ServerReflectionResponse {
        val reply = ServerReflectionResponse(valid_host = request.host, original_request = request)
        return when {
            request.list_services != null -> {
                reply.copy(list_services_response = ListServiceResponse(service = services.map { ServiceResponse(name = it) }))
            }

            request.file_by_filename != null -> {
                files(request.file_by_filename)?.let { reply.copy(file_descriptor_response = it) }
                    ?: reply.error(StatusCode.NOT_FOUND, "unknown file")
            }

            request.file_containing_symbol != null -> {
                GrpcSchema.symbols[request.file_containing_symbol]?.let(::files)?.let { reply.copy(file_descriptor_response = it) }
                    ?: reply.error(StatusCode.NOT_FOUND, "unknown symbol")
            }

            request.all_extension_numbers_of_type != null -> {
                if (request.all_extension_numbers_of_type in GrpcSchema.symbols) {
                    // aulo's protos declare no extensions.
                    reply.copy(
                        all_extension_numbers_response = ExtensionNumberResponse(base_type_name = request.all_extension_numbers_of_type),
                    )
                } else {
                    reply.error(StatusCode.NOT_FOUND, "unknown type")
                }
            }

            request.file_containing_extension != null -> {
                reply.error(StatusCode.NOT_FOUND, "unknown extension")
            }

            else -> {
                reply.error(StatusCode.INVALID_ARGUMENT, "empty request")
            }
        }
    }

    /** [name] and every file it imports, transitively: the client needs them all to resolve types. */
    private fun files(name: String): FileDescriptorResponse? {
        if (name !in GrpcSchema.descriptors) return null
        val order = LinkedHashSet<String>()

        fun visit(file: String) {
            if (order.add(file)) GrpcSchema.imports[file].orEmpty().forEach(::visit)
        }
        visit(name)
        return FileDescriptorResponse(file_descriptor_proto = order.map(descriptors::getValue))
    }

    private fun ServerReflectionResponse.error(
        code: StatusCode,
        message: String,
    ) = copy(error_response = ErrorResponse(error_code = code.value, error_message = message))
}
