package aulo.chat.data

import kotlinx.rpc.grpc.GrpcStatus

// kotlinx-rpc declares the status description internal in common code and public on each
// platform, so only platform code can read it.
internal expect fun GrpcStatus.detail(): String?
