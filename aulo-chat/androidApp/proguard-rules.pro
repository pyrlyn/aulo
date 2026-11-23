# kotlinx-rpc reaches its generated code by reflection (a service's descriptor through an
# annotation on the service interface, read with kotlin-reflect) and ships no shrinker rules
# yet: https://github.com/Kotlin/kotlinx-rpc/issues/330. Until it does, its runtime and the
# generated aulo.v1 contract are kept whole; the app's own code is still shrunk and renamed.
-keep class kotlinx.rpc.** { *; }
-keep class aulo.v1.** { *; }

# The descriptor of a streaming call names its type with typeOf<Flow<...>>(), and kotlin-reflect
# needs Flow's type parameter for that; R8 drops it from a class no rule mentions.
-keep,allowobfuscation,allowshrinking interface kotlinx.coroutines.flow.Flow
