@file:OptIn(ExperimentalSerializationApi::class)

package aulo.config

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.descriptors.PolymorphicKind
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.SerialKind
import kotlinx.serialization.descriptors.StructureKind
import kotlinx.serialization.descriptors.elementDescriptors
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonClassDiscriminator
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.doubleOrNull
import kotlinx.serialization.json.longOrNull

// The config types' serial descriptors are the one description of the config shape: layers are
// checked against them before anything is decoded, and the JSON Schema is generated from them.

internal fun join(
    path: String,
    key: String,
) = if (path.isEmpty()) key else "$path.$key"

internal fun SerialDescriptor.discriminator(): String =
    annotations.filterIsInstance<JsonClassDiscriminator>().firstOrNull()?.discriminator ?: "type"

/** The subclasses of a sealed descriptor, by serial name. */
internal fun SerialDescriptor.variants(): List<SerialDescriptor> = getElementDescriptor(1).elementDescriptors.toList()

internal fun SerialDescriptor.isInteger() =
    kind == PrimitiveKind.BYTE || kind == PrimitiveKind.SHORT || kind == PrimitiveKind.INT || kind == PrimitiveKind.LONG

/** The descriptor a dotted key leads to (map keys are free), or null when no such key exists. */
internal fun SerialDescriptor.at(segments: List<String>): SerialDescriptor? {
    var d = this
    for (segment in segments) {
        d =
            when (d.kind) {
                StructureKind.CLASS -> {
                    d.getElementIndex(segment).takeIf { it >= 0 }?.let(d::getElementDescriptor)
                }

                StructureKind.MAP -> {
                    d.getElementDescriptor(1)
                }

                PolymorphicKind.SEALED -> {
                    if (segment == d.discriminator()) {
                        d.getElementDescriptor(0)
                    } else {
                        d.variants().firstNotNullOfOrNull { v ->
                            v.getElementIndex(segment).takeIf { it >= 0 }?.let(v::getElementDescriptor)
                        }
                    }
                }

                else -> {
                    null
                }
            } ?: return null
    }
    return d
}

/**
 * Checks [element] against [descriptor]: unknown keys, JSON types, enum names and [Minimum].
 * With [complete], required keys must be present too; a single layer may leave them to another.
 * [originOf] names the layer behind a key for the error.
 */
internal class ShapeCheck(
    private val complete: Boolean,
    private val originOf: (String) -> Origin?,
) {
    fun check(
        element: JsonElement,
        descriptor: SerialDescriptor,
        path: String,
        annotations: List<Annotation> = emptyList(),
    ) {
        if (element is JsonNull) {
            if (!descriptor.isNullable) fail(path, "`$path` must not be null")
            return
        }
        when (val kind = descriptor.kind) {
            StructureKind.CLASS, StructureKind.OBJECT -> {
                checkClass(table(element, path), descriptor, path, skip = null)
            }

            PolymorphicKind.SEALED -> {
                checkSealed(table(element, path), descriptor, path)
            }

            StructureKind.LIST -> {
                val array = element as? JsonArray ?: fail(path, "`$path` must be an array")
                array.forEachIndexed { i, item -> check(item, descriptor.getElementDescriptor(0), "$path[$i]") }
            }

            StructureKind.MAP -> {
                table(element, path).forEach { (key, value) -> check(value, descriptor.getElementDescriptor(1), join(path, key)) }
            }

            SerialKind.ENUM -> {
                val ok = element is JsonPrimitive && element.isString && descriptor.getElementIndex(element.content) >= 0
                if (!ok) fail(path, "`$path` must be one of ${descriptor.elementNames.joinToString { "\"$it\"" }}")
            }

            PrimitiveKind.STRING -> {
                if (!(element is JsonPrimitive && element.isString)) fail(path, "`$path` must be a string")
            }

            PrimitiveKind.BOOLEAN -> {
                if (!(element is JsonPrimitive && !element.isString && element.booleanOrNull != null)) {
                    fail(
                        path,
                        "`$path` must be a boolean",
                    )
                }
            }

            PrimitiveKind.FLOAT, PrimitiveKind.DOUBLE -> {
                if (!(element is JsonPrimitive && !element.isString && element.booleanOrNull == null && element.doubleOrNull != null)) {
                    fail(path, "`$path` must be a number")
                }
            }

            else -> {
                if (descriptor.isInteger()) {
                    checkInteger(element, path, annotations)
                } else {
                    fail(path, "`$path` has an unsupported type $kind")
                }
            }
        }
    }

    private fun checkInteger(
        element: JsonElement,
        path: String,
        annotations: List<Annotation>,
    ) {
        val value =
            (element as? JsonPrimitive)
                ?.takeIf { !it.isString && it.content.none { c -> c == '.' || c == 'e' || c == 'E' } }
                ?.longOrNull ?: fail(path, "`$path` must be an integer")
        annotations.filterIsInstance<Minimum>().firstOrNull()?.let {
            if (value < it.value) fail(path, "`$path` must be at least ${it.value}")
        }
    }

    private fun checkClass(
        table: JsonObject,
        descriptor: SerialDescriptor,
        path: String,
        skip: String?,
    ) {
        for ((key, value) in table) {
            if (key == skip) continue
            val index = descriptor.getElementIndex(key)
            if (index < 0) fail(join(path, key), "unknown key `${join(path, key)}`")
            check(value, descriptor.getElementDescriptor(index), join(path, key), descriptor.getElementAnnotations(index))
        }
        if (!complete) return
        for (i in 0 until descriptor.elementsCount) {
            val name = descriptor.getElementName(i)
            if (!descriptor.isElementOptional(i) && name !in table) fail(path, "missing key `${join(path, name)}`")
        }
    }

    private fun checkSealed(
        table: JsonObject,
        descriptor: SerialDescriptor,
        path: String,
    ) {
        val name = descriptor.discriminator()
        val variants = descriptor.variants()
        val tag = table[name]
        if (tag == null) {
            if (complete) fail(path, "missing key `${join(path, name)}`")
            // A partial layer cannot pick the variant, so it may use any variant's keys.
            for ((key, value) in table) {
                val owner =
                    variants.firstOrNull { it.getElementIndex(key) >= 0 } ?: fail(join(path, key), "unknown key `${join(path, key)}`")
                val index = owner.getElementIndex(key)
                check(value, owner.getElementDescriptor(index), join(path, key), owner.getElementAnnotations(index))
            }
            return
        }
        val variant =
            variants.firstOrNull { tag is JsonPrimitive && tag.isString && it.serialName == tag.content }
                ?: fail(join(path, name), "`${join(path, name)}` must be one of ${variants.joinToString { "\"${it.serialName}\"" }}")
        checkClass(table, variant, path, skip = name)
    }

    private fun table(
        element: JsonElement,
        path: String,
    ) = element as? JsonObject ?: fail(path, "`$path` must be a table")

    private fun fail(
        key: String,
        detail: String,
    ): Nothing = throw ConfigException(originOf(key), key, detail)
}
