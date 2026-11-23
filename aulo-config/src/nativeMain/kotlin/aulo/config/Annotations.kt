package aulo.config

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.SerialInfo

/** Description of a config key or section; the schema generator copies it into `description`. */
@OptIn(ExperimentalSerializationApi::class)
@SerialInfo
@Target(AnnotationTarget.PROPERTY, AnnotationTarget.CLASS)
annotation class Doc(
    val text: String,
)

/** Smallest allowed integer; checked on every layer and written to the schema as `minimum`. */
@OptIn(ExperimentalSerializationApi::class)
@SerialInfo
@Target(AnnotationTarget.PROPERTY)
annotation class Minimum(
    val value: Long,
)
