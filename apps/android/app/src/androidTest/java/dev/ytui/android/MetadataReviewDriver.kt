package dev.ytui.android

import android.graphics.BitmapFactory
import android.os.ParcelFileDescriptor
import android.view.View
import android.view.ViewGroup
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsOwner
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.*

/** Read laid-out text without requiring a continuously repainting media screen to be idle. */
internal class MetadataReviewDriver(private val scenario: ActivityScenario<RustActivity>) {
    private fun inspect(check: (SemanticsNode) -> Boolean): Boolean {
        var found = false
        scenario.onActivity { activity ->
            fun walk(node: SemanticsNode) {
                val bounds = node.boundsInWindow
                if (bounds.width > 0 && bounds.height > 0 && bounds.bottom > 0 && bounds.top < activity.window.decorView.height && check(node)) found = true
                node.children.forEach(::walk)
            }
            fun views(view: View) {
                val getter = view.javaClass.methods.firstOrNull { it.name == "getSemanticsOwner" && it.parameterCount == 0 }
                (getter?.invoke(view) as? SemanticsOwner)?.let { walk(it.unmergedRootSemanticsNode) }
                if (view is ViewGroup) for (i in 0 until view.childCount) views(view.getChildAt(i))
            }
            views(activity.window.decorView)
        }
        return found
    }

    fun visibleText(text: String, exact: Boolean = true) {
        try {
            MediaRuntimeTest.await("Visible metadata: $text") {
                inspect { node -> node.config.getOrNull(SemanticsProperties.Text)?.any {
                    if (exact) it.text == text else it.text.contains(text)
                } == true }
            }
        } catch (failure: AssertionError) {
            val actual = mutableListOf<String>()
            inspect { node ->
                node.config.getOrNull(SemanticsProperties.Text)?.forEach { actual += it.text }
                false
            }
            throw AssertionError("$text not visible; laid-out text: $actual", failure)
        }
    }

    fun description(click: Boolean = false) {
        MediaRuntimeTest.await("Visible description preview") {
            inspect { node ->
                if (node.config.getOrNull(SemanticsProperties.TestTag) != "video-description") false
                else if (click) node.config.getOrNull(SemanticsActions.OnClick)?.action?.invoke() == true
                else true
            }
        }
    }

    fun capture(name: String) {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val file = File(instrumentation.targetContext.getExternalFilesDir(null), name)
        ParcelFileDescriptor.AutoCloseInputStream(instrumentation.uiAutomation.executeShellCommand("screencap -p")).use { input ->
            file.outputStream().use { input.copyTo(it) }
        }
        val bitmap = BitmapFactory.decodeFile(file.absolutePath)
        assertNotNull("A real screenshot must be saved: $name", bitmap)
        assertTrue(bitmap!!.width > 0 && bitmap.height > 0)
        bitmap.recycle()
    }
}
