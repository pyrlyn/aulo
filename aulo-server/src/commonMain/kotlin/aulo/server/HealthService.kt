package aulo.server

import aulo.grpc.GrpcException
import aulo.grpc.StatusCode
import io.grpc.health.v1.HealthCheckRequest
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.health.v1.HealthCheckResponse.ServingStatus
import io.grpc.health.v1.HealthListRequest
import io.grpc.health.v1.HealthListResponse
import io.grpc.health.v1.HealthServer
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.transformWhile
import kotlinx.coroutines.flow.update

/**
 * `grpc.health.v1.Health`. The empty service name is the server as a whole. Once [shutDown] runs,
 * everything reads NOT_SERVING and open Watch streams end, so they do not hold up a graceful
 * shutdown.
 */
class HealthService(
    services: Collection<String> = emptyList(),
) : HealthServer {
    private class State(
        val statuses: Map<String, ServingStatus>,
        val terminal: Boolean = false,
    )

    private val state = MutableStateFlow(State((services + "").associateWith { ServingStatus.SERVING }))

    /** Sets one service's status; ignored after [shutDown]. */
    fun set(
        service: String,
        status: ServingStatus,
    ) = state.update { if (it.terminal) it else State(it.statuses + (service to status)) }

    fun shutDown() = state.update { State(it.statuses.mapValues { ServingStatus.NOT_SERVING }, terminal = true) }

    override suspend fun Check(request: HealthCheckRequest): HealthCheckResponse {
        val status = state.value.statuses[request.service] ?: throw GrpcException(StatusCode.NOT_FOUND, "unknown service")
        return HealthCheckResponse(status = status)
    }

    override suspend fun List(request: HealthListRequest): HealthListResponse =
        HealthListResponse(statuses = state.value.statuses.mapValues { HealthCheckResponse(status = it.value) })

    override suspend fun Watch(
        request: HealthCheckRequest,
        response: SendChannel<HealthCheckResponse>,
    ) {
        state
            .transformWhile {
                emit(it.statuses[request.service] ?: ServingStatus.SERVICE_UNKNOWN)
                !it.terminal
            }.distinctUntilChanged()
            .collect { response.send(HealthCheckResponse(status = it)) }
    }
}
