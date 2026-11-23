package aulo.chat.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import aulo.chat.ui.App

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // The screen pads itself by the system bars and the keyboard.
        enableEdgeToEdge()
        val version = packageManager.getPackageInfo(packageName, 0).versionName.orEmpty()
        setContent { App(version) }
    }
}
