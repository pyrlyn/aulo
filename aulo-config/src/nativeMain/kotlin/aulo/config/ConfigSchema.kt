@file:OptIn(ExperimentalSerializationApi::class)

package aulo.config

import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.descriptors.PolymorphicKind
import kotlinx.serialization.descriptors.PrimitiveKind
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.descriptors.SerialKind
import kotlinx.serialization.descriptors.StructureKind
import kotlinx.serialization.descriptors.elementNames
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive

/**
 * JSON Schema (draft 2020-12) of `config.toml`, generated from the config types so it cannot
 * drift: schemas/aulo.schema.json is this output, and a test fails when the two differ. Keys
 * are sorted so the file only changes when the types do.
 */
object ConfigSchema {
    private val pretty =
        Json {
            prettyPrint = true
            prettyPrintIndent = "  "
        }

    fun generate(): String {
        val defaults = configJson.encodeToJsonElement(Config.serializer(), Config()) as JsonObject
        val generator = Generator()
        val root = generator.classSchema(Config.serializer().descriptor, defaults).toMutableMap()
        root["\$schema"] = JsonPrimitive("https://json-schema.org/draft/2020-12/schema")
        root["title"] = JsonPrimitive("Config")
        root["\$defs"] = JsonObject(generator.defs)
        return pretty.encodeToString(JsonElement.serializer(), sorted(JsonObject(root))) + "\n"
    }

    private class Generator {
        val defs = mutableMapOf<String, JsonElement>()

        fun classSchema(
            descriptor: SerialDescriptor,
            defaults: JsonObject?,
            discriminator: Pair<String, String>? = null,
        ): JsonObject {
            val properties = mutableMapOf<String, JsonElement>()
            val required = mutableListOf<String>()
            discriminator?.let { (name, value) ->
                properties[name] = obj("const" to JsonPrimitive(value), "type" to JsonPrimitive("string"))
                required += name
            }
            for (i in 0 until descriptor.elementsCount) {
                val name = descriptor.getElementName(i)
                val schema =
                    schemaOf(descriptor.getElementDescriptor(i), defaults?.get(name)).toMutableMap()
                descriptor.getElementAnnotations(i).forEach {
                    when (it) {
                        is Doc -> schema["description"] = JsonPrimitive(it.text)
                        is Minimum -> schema["minimum"] = JsonPrimitive(it.value)
                    }
                }
                defaults?.get(name)?.let { schema["default"] = it }
                properties[name] = JsonObject(schema)
                if (!descriptor.isElementOptional(i)) required += name
            }
            val out =
                mutableMapOf<String, JsonElement>(
                    "type" to JsonPrimitive("object"),
                    "additionalProperties" to JsonPrimitive(false),
                    "properties" to JsonObject(properties),
                )
            if (required.isNotEmpty()) out["required"] = JsonArray(required.map(::JsonPrimitive))
            doc(descriptor)?.let { out["description"] = JsonPrimitive(it) }
            return JsonObject(out)
        }

        private fun schemaOf(
            descriptor: SerialDescriptor,
            default: JsonElement?,
        ): JsonObject =
            when (descriptor.kind) {
                StructureKind.CLASS -> {
                    ref(descriptor) { classSchema(descriptor, default as? JsonObject) }
                }

                PolymorphicKind.SEALED -> {
                    ref(descriptor) {
                        val name = descriptor.discriminator()
                        obj("oneOf" to JsonArray(descriptor.variants().map { classSchema(it, null, name to it.serialName) }))
                    }
                }

                SerialKind.ENUM -> {
                    ref(descriptor) {
                        val out =
                            mutableMapOf(
                                "enum" to JsonArray(descriptor.elementNames.map(::JsonPrimitive)),
                                "type" to JsonPrimitive("string"),
                            )
                        doc(descriptor)?.let { out["description"] = JsonPrimitive(it) }
                        JsonObject(out)
                    }
                }

                StructureKind.LIST -> {
                    obj("type" to JsonPrimitive("array"), "items" to schemaOf(descriptor.getElementDescriptor(0), null))
                }

                StructureKind.MAP -> {
                    obj(
                        "type" to JsonPrimitive("object"),
                        "additionalProperties" to schemaOf(descriptor.getElementDescriptor(1), null),
                    )
                }

                PrimitiveKind.STRING -> {
                    obj("type" to JsonPrimitive("string"))
                }

                PrimitiveKind.BOOLEAN -> {
                    obj("type" to JsonPrimitive("boolean"))
                }

                PrimitiveKind.FLOAT, PrimitiveKind.DOUBLE -> {
                    obj("type" to JsonPrimitive("number"))
                }

                else -> {
                    if (descriptor.isInteger()) {
                        obj("type" to JsonPrimitive("integer"))
                    } else {
                        error("no schema for ${descriptor.serialName} (${descriptor.kind})")
                    }
                }
            }

        // Named types go to $defs once, under their simple name; the first use supplies defaults.
        private fun ref(
            descriptor: SerialDescriptor,
            build: () -> JsonObject,
        ): JsonObject {
            val name = descriptor.serialName.removeSuffix("?").substringAfterLast('.')
            if (name !in defs) {
                defs[name] = JsonObject(emptyMap())
                defs[name] = build()
            }
            return obj("\$ref" to JsonPrimitive("#/\$defs/$name"))
        }

        private fun doc(descriptor: SerialDescriptor) =
            descriptor.annotations
                .filterIsInstance<Doc>()
                .firstOrNull()
                ?.text
    }

    private fun obj(vararg entries: Pair<String, JsonElement>) = JsonObject(mapOf(*entries))

    private fun sorted(element: JsonElement): JsonElement =
        when (element) {
            is JsonObject -> JsonObject(element.entries.sortedBy { it.key }.associate { it.key to sorted(it.value) })
            is JsonArray -> JsonArray(element.map(::sorted))
            else -> element
        }
}
