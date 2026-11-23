package aulo.chat.data

import kotlinx.rpc.grpc.GrpcStatus

internal actual fun GrpcStatus.detail(): String? = getDescription()
