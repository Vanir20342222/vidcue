package dev.ytui.android

import android.graphics.Bitmap
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.IndicationNodeFactory
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.indication
import androidx.compose.foundation.interaction.FocusInteraction
import androidx.compose.foundation.interaction.HoverInteraction
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.node.CompositionLocalConsumerModifierNode
import androidx.compose.ui.node.DelegatableNode
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.currentValueOf
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONObject

/**
 * The ytui "screening room" design language: the interface recedes so artwork is the light in
 * the room. One expressive display face (Bricolage Grotesque) names places and what is playing;
 * a calm text face (Instrument Sans) carries everything else. Palette colours still come from the
 * shared Rust themes; this file only decides how they are used.
 */
internal object YtuiType {
    @OptIn(ExperimentalTextApi::class)
    private fun display(weight: Int, opsz: Float) =
        Font(
            R.font.bricolage_grotesque,
            FontWeight(weight),
            variationSettings =
                FontVariation.Settings(
                    FontVariation.weight(weight),
                    FontVariation.Setting("opsz", opsz),
                    FontVariation.width(100f),
                ),
        )

    @OptIn(ExperimentalTextApi::class)
    private fun text(weight: Int) =
        Font(
            R.font.instrument_sans,
            FontWeight(weight),
            variationSettings =
                FontVariation.Settings(FontVariation.weight(weight), FontVariation.width(100f)),
        )

    /** Large optical size: page names, the playing title. */
    val display = FontFamily(display(500, 48f), display(600, 48f), display(700, 48f), display(800, 48f))

    /** Small optical size of the same face: sheet and section titles. */
    val title = FontFamily(display(500, 18f), display(600, 18f), display(700, 18f), display(800, 18f))

    val text = FontFamily(text(400), text(500), text(600), text(700))

    val typography =
        androidx.compose.material3.Typography(
            displaySmall =
                TextStyle(
                    fontFamily = display,
                    fontWeight = FontWeight.Bold,
                    fontSize = 34.sp,
                    lineHeight = 38.sp,
                    letterSpacing = (-0.8).sp,
                ),
            headlineMedium =
                TextStyle(
                    fontFamily = display,
                    fontWeight = FontWeight.Bold,
                    fontSize = 28.sp,
                    lineHeight = 32.sp,
                    letterSpacing = (-0.6).sp,
                ),
            headlineSmall =
                TextStyle(
                    fontFamily = display,
                    fontWeight = FontWeight.Bold,
                    fontSize = 24.sp,
                    lineHeight = 28.sp,
                    letterSpacing = (-0.4).sp,
                ),
            titleLarge =
                TextStyle(
                    fontFamily = title,
                    fontWeight = FontWeight.Bold,
                    fontSize = 20.sp,
                    lineHeight = 25.sp,
                    letterSpacing = (-0.2).sp,
                ),
            titleMedium =
                TextStyle(
                    fontFamily = text,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 16.sp,
                    lineHeight = 22.sp,
                ),
            titleSmall =
                TextStyle(
                    fontFamily = text,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 15.sp,
                    lineHeight = 20.sp,
                ),
            bodyLarge = TextStyle(fontFamily = text, fontSize = 16.sp, lineHeight = 23.sp),
            bodyMedium = TextStyle(fontFamily = text, fontSize = 14.sp, lineHeight = 20.sp),
            bodySmall = TextStyle(fontFamily = text, fontSize = 13.sp, lineHeight = 18.sp),
            labelLarge =
                TextStyle(
                    fontFamily = text,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 14.sp,
                    lineHeight = 18.sp,
                    letterSpacing = 0.1.sp,
                ),
            labelMedium =
                TextStyle(
                    fontFamily = text,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 12.5.sp,
                    lineHeight = 16.sp,
                    letterSpacing = 0.1.sp,
                ),
            labelSmall =
                TextStyle(
                    fontFamily = text,
                    fontWeight = FontWeight.SemiBold,
                    fontSize = 11.5.sp,
                    lineHeight = 14.sp,
                    letterSpacing = 0.2.sp,
                ),
        )

    @OptIn(ExperimentalTextApi::class)
    private fun dotFont(weight: Int) =
        Font(
            R.font.doto,
            FontWeight(weight),
            // Fully round dots, like Nothing's Ndot dot-matrix face.
            variationSettings =
                FontVariation.Settings(
                    FontVariation.weight(weight),
                    FontVariation.Setting("ROND", 100f),
                ),
        )

    @OptIn(ExperimentalTextApi::class)
    private fun geistFont(weight: Int) =
        Font(
            R.font.geist,
            FontWeight(weight),
            variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
        )

    @OptIn(ExperimentalTextApi::class)
    private fun geistMonoFont(weight: Int) =
        Font(
            R.font.geist_mono,
            FontWeight(weight),
            variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
        )

    /** Dot-matrix display face (Doto, an open stand-in for Nothing's Ndot). */
    val dotMatrix = FontFamily(dotFont(700), dotFont(800), dotFont(900))

    /** Nothing OS 5's everyday sans and its monospace companion. */
    val geist = FontFamily(geistFont(400), geistFont(500), geistFont(600), geistFont(700))
    val geistMono = FontFamily(geistMonoFont(400), geistMonoFont(500), geistMonoFont(600))

    /**
     * Nothing-styled type: dot matrix only where it gives character (page names and the
     * playing title), Geist for reading, and Geist Mono for small labels and metadata.
     */
    val nothing: androidx.compose.material3.Typography =
        typography.run {
            fun TextStyle.dots() =
                copy(fontFamily = dotMatrix, fontWeight = FontWeight.ExtraBold, letterSpacing = 0.sp)
            fun TextStyle.mono(spacing: Float) =
                copy(fontFamily = geistMono, fontWeight = FontWeight.Medium, letterSpacing = spacing.sp)
            copy(
                displaySmall = displaySmall.dots(),
                headlineMedium = headlineMedium.dots(),
                headlineSmall = headlineSmall.dots(),
                titleLarge =
                    titleLarge.copy(
                        fontFamily = geist,
                        fontWeight = FontWeight.SemiBold,
                        letterSpacing = 0.sp,
                    ),
                titleMedium = titleMedium.copy(fontFamily = geist, fontWeight = FontWeight.Medium),
                titleSmall = titleSmall.copy(fontFamily = geist, fontWeight = FontWeight.Medium),
                bodyLarge = bodyLarge.copy(fontFamily = geist),
                bodyMedium = bodyMedium.copy(fontFamily = geist),
                bodySmall = bodySmall.copy(fontFamily = geist),
                labelLarge = labelLarge.mono(0.4f),
                labelMedium = labelMedium.mono(0.4f),
                labelSmall = labelSmall.mono(0.5f),
            )
        }

    /** Type for an applied palette: built-in Nothing palettes get [nothing]; others the default. */
    fun forPalette(palette: JSONObject?): androidx.compose.material3.Typography =
        if (palette != null && isDotMatrix(palette.optString("name"), palette.optString("source")))
            nothing
        else typography

    /** Mirrors Rust's `Palette::dot_matrix`: imported palettes never switch type. */
    fun isDotMatrix(name: String, source: String): Boolean =
        source == "Built-in" && name.startsWith("nothing-")
}

/**
 * Press, hover and keyboard focus feedback that replaces Material's ripple: a soft wash of the
 * current ink that blooms quickly and fades slowly, plus a visible ring for keyboard focus.
 */
internal object SoftIndication : IndicationNodeFactory {
    override fun create(interactionSource: InteractionSource): DelegatableNode =
        SoftIndicationNode(interactionSource)

    override fun equals(other: Any?) = other === this

    override fun hashCode() = 0x50f7
}

private class SoftIndicationNode(private val source: InteractionSource) :
    Modifier.Node(), DrawModifierNode, CompositionLocalConsumerModifierNode {
    private val press = Animatable(0f)
    private var hovered = false
    private var focused = false

    override fun onAttach() {
        coroutineScope.launch {
            var presses = 0
            source.interactions.collect { interaction ->
                when (interaction) {
                    is PressInteraction.Press -> {
                        presses++
                        launch { press.animateTo(1f, tween(70)) }
                    }
                    is PressInteraction.Release,
                    is PressInteraction.Cancel -> {
                        presses = (presses - 1).coerceAtLeast(0)
                        if (presses == 0)
                            launch {
                                // A quick tap still shows its full bloom before fading.
                                if (press.value < 1f) press.animateTo(1f, tween(60))
                                press.animateTo(0f, tween(360))
                            }
                    }
                    is HoverInteraction.Enter -> hovered = true
                    is HoverInteraction.Exit -> hovered = false
                    is FocusInteraction.Focus -> focused = true
                    is FocusInteraction.Unfocus -> focused = false
                }
                invalidateDraw()
            }
        }
    }

    override fun ContentDrawScope.draw() {
        drawContent()
        val ink = currentValueOf(LocalContentColor)
        val wash = press.value * 0.12f + (if (hovered) 0.05f else 0f) + (if (focused) 0.08f else 0f)
        if (wash > 0f) drawRect(ink.copy(alpha = wash.coerceAtMost(0.22f)))
        if (focused) {
            val width = 2.dp.toPx()
            drawRoundRect(
                ink.copy(alpha = 0.85f),
                Offset(width / 2, width / 2),
                Size(size.width - width, size.height - width),
                CornerRadius(minOf(size.height, size.width) / 2),
                style = Stroke(width),
            )
        }
    }
}

/** Springs the content in while pressed; reads the same interactions as [SoftIndication]. */
@Composable
internal fun Modifier.pressScale(source: InteractionSource, pressed: Float = 0.96f): Modifier {
    val motion = LocalMotion.current
    val down by source.collectIsPressedAsState()
    val scale by
        animateFloatAsState(
            if (down && motion.animate) pressed else 1f,
            motion.spring(),
            label = "press-scale",
        )
    return graphicsLayer {
        scaleX = scale
        scaleY = scale
    }
}

internal enum class PillStyle {
    /** Ink-filled: the one main action on a screen. */
    Primary,
    /** Raised neutral fill. */
    Tonal,
    /** Text only, a wash on press. */
    Quiet,
    /** Text only in the error colour. */
    Destructive,
}

/** Shared pill button. Keeps a 48dp touch height; the visible pill can be shorter. */
@Composable
internal fun Pill(
    label: String,
    modifier: Modifier = Modifier,
    style: PillStyle = PillStyle.Tonal,
    enabled: Boolean = true,
    glyph: Glyph? = null,
    fill: Boolean = false,
    onClick: () -> Unit,
) {
    val colors = MaterialTheme.colorScheme
    val container =
        when (style) {
            PillStyle.Primary -> colors.onBackground
            PillStyle.Tonal -> colors.surfaceContainerHigh
            else -> MediaTokens.transparent
        }
    val ink =
        when (style) {
            PillStyle.Primary -> colors.background
            PillStyle.Destructive -> colors.error
            else -> colors.onSurface
        }
    val source = remember { MutableInteractionSource() }
    CompositionLocalProvider(LocalContentColor provides ink.copy(alpha = if (enabled) 1f else 0.42f)) {
        Row(
            modifier
                .pressScale(source)
                .heightIn(min = 48.dp)
                .then(if (fill) Modifier.fillMaxWidth() else Modifier)
                .clip(MediaTokens.pill)
                .background(
                    if (enabled || style != PillStyle.Primary) container
                    else container.copy(alpha = 0.28f)
                )
                .clickable(
                    source,
                    SoftIndication,
                    enabled = enabled,
                    role = Role.Button,
                    onClick = onClick,
                )
                .padding(
                    horizontal = if (style == PillStyle.Quiet || style == PillStyle.Destructive) 14.dp else 20.dp,
                    vertical = 10.dp,
                ),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        ) {
            if (glyph != null) MediaIcon(glyph, Modifier.size(18.dp))
            Text(
                label,
                style = MaterialTheme.typography.labelLarge,
                maxLines = 2,
                textAlign = TextAlign.Center,
            )
        }
    }
}

/** Round icon target with a soft wash and a small spring on press. */
@Composable
internal fun RoundIconButton(
    glyph: Glyph,
    label: String,
    modifier: Modifier = Modifier,
    size: Dp = 48.dp,
    iconSize: Dp = 24.dp,
    container: Color = MediaTokens.transparent,
    tint: Color = LocalContentColor.current,
    enabled: Boolean = true,
    action: () -> Unit,
) {
    val source = remember { MutableInteractionSource() }
    CompositionLocalProvider(LocalContentColor provides tint) {
        Box(
            modifier
                .size(size)
                .pressScale(source, 0.9f)
                .clip(CircleShape)
                .background(container)
                .clickable(source, SoftIndication, enabled = enabled, role = Role.Button, onClick = action)
                .semantics { contentDescription = label },
            contentAlignment = Alignment.Center,
        ) {
            MediaIcon(glyph, Modifier.size(iconSize), tint.copy(alpha = if (enabled) tint.alpha else 0.4f))
        }
    }
}

/** Two-state switch drawn for this app: a soft track and a thumb that springs across. */
@Composable
internal fun MediaSwitch(checked: Boolean, enabled: Boolean = true, modifier: Modifier = Modifier) {
    val motion = LocalMotion.current
    val colors = MaterialTheme.colorScheme
    val position by animateFloatAsState(if (checked) 1f else 0f, motion.spring(), label = "switch")
    val off = lerp(colors.background, colors.onBackground, 0.2f)
    val accent = colors.primary
    val thumbOff = lerp(colors.background, colors.onBackground, 0.78f)
    val thumbOn = mediaOnAccent(accent)
    Canvas(modifier.size(width = 48.dp, height = 28.dp).graphicsLayer { alpha = if (enabled) 1f else 0.4f }) {
        val radius = size.height / 2
        drawRoundRect(lerp(off, accent, position), cornerRadius = CornerRadius(radius))
        val thumb = radius - 3.dp.toPx() + position * 1.dp.toPx()
        val x = radius + (size.width - radius * 2) * position
        drawCircle(lerp(thumbOff, thumbOn, position), thumb, Offset(x, radius))
    }
}

/** Selection mark for single-choice lists: an outlined ring that fills with a check. */
@Composable
internal fun ChoiceMark(selected: Boolean, modifier: Modifier = Modifier) {
    val motion = LocalMotion.current
    val colors = MaterialTheme.colorScheme
    val fill by animateFloatAsState(if (selected) 1f else 0f, motion.spring(), label = "choice")
    val ring = colors.outline
    val accent = colors.primary
    val check = mediaOnAccent(accent)
    Canvas(modifier.size(22.dp)) {
        val r = size.minDimension / 2
        drawCircle(ring, r - 1.dp.toPx(), style = Stroke(1.5.dp.toPx()))
        if (fill > 0f) {
            drawCircle(accent, r * fill)
            val s = size.width / 22f
            drawLine(check.copy(alpha = fill), Offset(6.5f * s, 11.5f * s), Offset(9.6f * s, 14.6f * s), 2.dp.toPx(), StrokeCap.Round)
            drawLine(check.copy(alpha = fill), Offset(9.6f * s, 14.6f * s), Offset(15.8f * s, 8f * s), 2.dp.toPx(), StrokeCap.Round)
        }
    }
}

/** Input with its label inside the field, so the label never floats or jumps. */
@Composable
internal fun FieldFrame(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    modifier: Modifier = Modifier,
    singleLine: Boolean = true,
    minLines: Int = 1,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    height: Dp? = null,
) {
    val colors = MaterialTheme.colorScheme
    val source = remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    val motion = LocalMotion.current
    val edge by
        animateColorAsState(
            if (focused) colors.onSurface.copy(alpha = 0.7f) else colors.onSurface.copy(alpha = 0f),
            motion.fade(160),
            label = "field-edge",
        )
    BasicTextField(
        value,
        onValueChange,
        modifier.fillMaxWidth(),
        textStyle = MaterialTheme.typography.bodyLarge.copy(color = colors.onSurface),
        cursorBrush = SolidColor(colors.primary),
        singleLine = singleLine,
        minLines = minLines,
        keyboardOptions = keyboardOptions,
        keyboardActions = keyboardActions,
        visualTransformation = visualTransformation,
        interactionSource = source,
        decorationBox = { inner ->
            Column(
                Modifier.fillMaxWidth()
                    .then(if (height != null) Modifier.height(height) else Modifier.heightIn(min = 60.dp))
                    .clip(MediaTokens.fieldShape)
                    // Ink-relative fill reads on the page, on sheets and inside settings cards.
                    .background(colors.onSurface.copy(alpha = 0.07f))
                    .border(1.5.dp, edge, MediaTokens.fieldShape)
                    .padding(horizontal = 16.dp, vertical = 10.dp),
                verticalArrangement = Arrangement.spacedBy(2.dp),
            ) {
                Text(
                    label,
                    style = MaterialTheme.typography.labelMedium,
                    color = colors.onSurfaceVariant,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Box(Modifier.fillMaxWidth()) { inner() }
            }
        },
    )
}

/** Overflow menu: the platform popup (so Back and outside taps dismiss) with app styling. */
@Composable
internal fun MediaMenu(expanded: Boolean, onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    DropdownMenu(
        expanded,
        onDismiss,
        modifier = Modifier.widthIn(min = 220.dp).padding(horizontal = 6.dp),
        shape = MediaTokens.menu,
        containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
        tonalElevation = 0.dp,
        shadowElevation = 18.dp,
        border = androidx.compose.foundation.BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
        content = content,
    )
}

@Composable
internal fun MediaMenuItem(
    text: String,
    glyph: Glyph,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    destructive: Boolean = false,
    onClick: () -> Unit,
) {
    val ink =
        if (destructive) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface
    CompositionLocalProvider(LocalContentColor provides ink.copy(alpha = if (enabled) 1f else 0.4f)) {
        Row(
            modifier
                .fillMaxWidth()
                .heightIn(min = 48.dp)
                .clip(MediaTokens.medium)
                .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
                .padding(horizontal = 12.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            MediaIcon(glyph, Modifier.size(20.dp))
            Text(text, style = MaterialTheme.typography.bodyLarge)
        }
    }
}

/** Thin rule between menu groups. */
@Composable
internal fun MenuRule() {
    Box(
        Modifier.padding(vertical = 4.dp, horizontal = 12.dp)
            .fillMaxWidth()
            .height(1.dp)
            .background(MaterialTheme.colorScheme.outlineVariant)
    )
}

/**
 * Centred dialog card that settles in with a short scale and fade. [actions] sit at the trailing
 * edge as pills; the first composable passed is the dismiss action.
 */
@Composable
internal fun MediaDialog(
    onDismiss: () -> Unit,
    title: String,
    modifier: Modifier = Modifier,
    titleModifier: Modifier = Modifier,
    actions: @Composable RowScope.() -> Unit = {},
    content: @Composable ColumnScope.() -> Unit,
) {
    Dialog(onDismiss, DialogProperties(usePlatformDefaultWidth = false)) {
        val motion = LocalMotion.current
        val enter = remember { Animatable(if (motion.animate) 0f else 1f) }
        LaunchedEffect(Unit) { enter.animateTo(1f, motion.enter(260)) }
        Column(
            modifier
                .padding(24.dp)
                .widthIn(max = 440.dp)
                .fillMaxWidth()
                .graphicsLayer {
                    alpha = enter.value
                    val scale = 0.94f + 0.06f * enter.value
                    scaleX = scale
                    scaleY = scale
                }
                .clip(MediaTokens.dialog)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .padding(top = 24.dp, bottom = 12.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) {
                Text(
                    title,
                    titleModifier.padding(horizontal = 24.dp),
                    style = MaterialTheme.typography.titleLarge,
                )
                Column(Modifier.weight(1f, fill = false).padding(horizontal = 24.dp), content = content)
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 12.dp),
                    horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.End),
                    verticalAlignment = Alignment.CenterVertically,
                    content = actions,
                )
            }
        }
    }
}

/** A drifting highlight for content that is still loading. Still under reduced motion. */
@Composable
internal fun Modifier.shimmer(shape: Shape = MediaTokens.medium): Modifier {
    val motion = LocalMotion.current
    val base = MaterialTheme.colorScheme.surfaceContainer
    val glint = lerp(base, MaterialTheme.colorScheme.onBackground, 0.07f)
    val phase =
        if (!motion.animate) remember { mutableStateOf(0.5f) }
        else
            rememberInfiniteTransition(label = "shimmer")
                .animateFloat(
                    -0.4f,
                    1.4f,
                    infiniteRepeatable(tween(1400, easing = LinearEasing), RepeatMode.Restart),
                    label = "shimmer-phase",
                )
    return clip(shape).drawWithContent {
        val x = size.width * phase.value
        drawRect(
            Brush.linearGradient(
                listOf(base, glint, base),
                start = Offset(x - size.width * 0.35f, 0f),
                end = Offset(x + size.width * 0.35f, size.height),
            )
        )
        drawContent()
    }
}

/** Small rotating arc used wherever the app waits. */
@Composable
internal fun MediaSpinner(modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.onSurface) {
    val motion = LocalMotion.current
    val turn =
        if (!motion.animate) remember { mutableStateOf(0f) }
        else
            rememberInfiniteTransition(label = "spinner")
                .animateFloat(0f, 360f, infiniteRepeatable(tween(900, easing = LinearEasing)), label = "turn")
    Canvas(modifier.size(28.dp)) {
        val stroke = 2.5.dp.toPx()
        drawCircle(color.copy(alpha = 0.14f), size.minDimension / 2 - stroke, style = Stroke(stroke))
        drawArc(
            color,
            turn.value - 90f,
            100f,
            false,
            Offset(stroke, stroke),
            Size(size.width - stroke * 2, size.height - stroke * 2),
            style = Stroke(stroke, cap = StrokeCap.Round),
        )
    }
}

/** Rounded progress line; `progress == null` shows an indeterminate sweep. */
@Composable
internal fun MediaProgressBar(
    progress: Float?,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.primary,
    track: Color = MaterialTheme.colorScheme.outlineVariant,
    thickness: Dp = 4.dp,
) {
    val motion = LocalMotion.current
    val sweep =
        if (progress != null || !motion.animate) remember { mutableStateOf(0.3f) }
        else
            rememberInfiniteTransition(label = "progress")
                .animateFloat(-0.3f, 1f, infiniteRepeatable(tween(1100, easing = LinearEasing)), label = "sweep")
    Canvas(modifier.fillMaxWidth().height(thickness)) {
        val r = CornerRadius(size.height / 2)
        drawRoundRect(track, cornerRadius = r)
        if (progress != null)
            drawRoundRect(color, size = Size(size.width * progress.coerceIn(0f, 1f), size.height), cornerRadius = r)
        else {
            val start = (sweep.value * size.width).coerceAtLeast(0f)
            val end = ((sweep.value + 0.3f) * size.width).coerceAtMost(size.width)
            if (end > start) drawRoundRect(color, Offset(start, 0f), Size(end - start, size.height), r)
        }
    }
}

/** Glyph in a soft round tile, for empty states and identity rows. */
@Composable
internal fun GlyphBadge(glyph: Glyph, size: Dp = 64.dp, tint: Color = MaterialTheme.colorScheme.onSurface) {
    Box(
        Modifier.size(size)
            .background(
                Brush.linearGradient(
                    listOf(
                        MaterialTheme.colorScheme.surfaceContainerHigh,
                        MaterialTheme.colorScheme.surfaceContainer,
                    )
                ),
                CircleShape,
            )
            .border(1.dp, MaterialTheme.colorScheme.outlineVariant, CircleShape),
        contentAlignment = Alignment.Center,
    ) {
        MediaIcon(glyph, Modifier.size(size * 0.42f), tint)
    }
}

/** Header used inside sheets and settings pages; sentence case, no capitals. */
@Composable
internal fun GroupLabel(title: String, modifier: Modifier = Modifier) {
    Text(
        title,
        modifier.padding(top = 20.dp, bottom = 4.dp, start = 4.dp),
        style = MaterialTheme.typography.titleSmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/**
 * The colour artwork casts on its surroundings. Dark palettes get a deep, saturated version of
 * the cover's average colour; light palettes a pale one. Returns [fallback] until art arrives.
 */
@Composable
internal fun rememberArtworkTint(media: MediaCard?, fallback: Color, enabled: Boolean = true): Color {
    val dark = MaterialTheme.colorScheme.background.luminance() < 0.4f
    var tint by remember(media?.key, dark) { mutableStateOf(media?.let { ArtworkTint.cached(it.key, dark) }) }
    LaunchedEffect(media?.key, media?.artworkSource, dark, enabled) {
        if (media == null || !enabled || tint != null) return@LaunchedEffect
        val bitmap = ArtworkCache.cached(media, 96) ?: runCatching { ArtworkCache.load(media, 96) }.getOrNull()
        if (bitmap != null) tint = withContext(Dispatchers.Default) { ArtworkTint.from(media.key, bitmap, dark) }
    }
    val motion = LocalMotion.current
    val shown by animateColorAsState(tint ?: fallback, motion.fade(600), label = "artwork-tint")
    return shown
}

internal object ArtworkTint {
    private val cache = android.util.LruCache<String, Int>(64)

    fun cached(key: String, dark: Boolean): Color? = cache.get("$dark:$key")?.let { Color(it) }

    fun from(key: String, bitmap: Bitmap, dark: Boolean): Color {
        // Sample a coarse grid; weight saturated pixels so a grey border does not win.
        var r = 0.0
        var g = 0.0
        var b = 0.0
        var weight = 0.0
        val step = maxOf(1, minOf(bitmap.width, bitmap.height) / 24)
        var y = 0
        while (y < bitmap.height) {
            var x = 0
            while (x < bitmap.width) {
                val p = bitmap.getPixel(x, y)
                val pr = (p shr 16 and 255) / 255.0
                val pg = (p shr 8 and 255) / 255.0
                val pb = (p and 255) / 255.0
                val max = maxOf(pr, pg, pb)
                val min = minOf(pr, pg, pb)
                val w = 0.15 + (max - min) * 2
                r += pr * w
                g += pg * w
                b += pb * w
                weight += w
                x += step
            }
            y += step
        }
        val hsv = FloatArray(3)
        android.graphics.Color.RGBToHSV(
            (r / weight * 255).toInt(),
            (g / weight * 255).toInt(),
            (b / weight * 255).toInt(),
            hsv,
        )
        hsv[1] = hsv[1].coerceIn(0.18f, 0.62f)
        hsv[2] = if (dark) hsv[2].coerceIn(0.30f, 0.42f) else 0.94f
        if (!dark) hsv[1] = hsv[1].coerceAtMost(0.22f)
        val color = android.graphics.Color.HSVToColor(hsv)
        cache.put("$dark:$key", color)
        return Color(color)
    }
}

/** A panel's ambient wash: the artwork tint at the top falling off into the page colour. */
internal fun Modifier.ambientGlow(tint: Color, background: Color, strength: Float = 1f): Modifier =
    drawWithContent {
        drawRect(background)
        drawRect(
            Brush.verticalGradient(
                0f to lerp(background, tint, 0.85f * strength),
                0.55f to lerp(background, tint, 0.32f * strength),
                1f to background,
            )
        )
        drawContent()
    }

/** Close control for sheets: a small raised disc inside a 48dp target. */
@Composable
internal fun SheetClose(dismiss: () -> Unit) {
    val source = remember { MutableInteractionSource() }
    Box(
        Modifier.size(48.dp)
            .pressScale(source, 0.88f)
            .clip(CircleShape)
            .clickable(source, null, role = Role.Button, onClick = dismiss)
            .testTag("panel-close")
            .semantics { contentDescription = "Close" },
        contentAlignment = Alignment.Center,
    ) {
        Box(
            Modifier.size(34.dp)
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.surfaceContainerHigh)
                .indication(source, SoftIndication),
            contentAlignment = Alignment.Center,
        ) {
            MediaIcon(Glyph.Close, Modifier.size(16.dp), MaterialTheme.colorScheme.onSurface)
        }
    }
}


/**
 * A theme in miniature: its page colour, a raised surface line, text and accent. "system" passes
 * an object with dark and light palettes and is drawn split diagonally.
 */
@Composable
internal fun PaletteSwatch(palette: JSONObject?, modifier: Modifier = Modifier) {
    val colors = MaterialTheme.colorScheme
    fun JSONObject.rgb(key: String, fallback: Color): Color =
        optJSONArray(key)?.let { Color(it.optInt(0), it.optInt(1), it.optInt(2)) } ?: fallback
    val halves =
        when {
            palette == null -> listOf(null)
            palette.has("dark") -> listOf(palette.optJSONObject("dark"), palette.optJSONObject("light"))
            else -> listOf(palette)
        }
    Canvas(
        modifier
            .size(width = 52.dp, height = 34.dp)
            .clip(MediaTokens.small)
            .border(1.dp, colors.outlineVariant, MediaTokens.small)
            .testTag("palette-swatch")
    ) {
        halves.forEachIndexed { index, p ->
            val bg = p?.rgb("background", colors.surfaceContainer) ?: colors.surfaceContainer
            val surface = p?.rgb("surface", colors.surfaceContainerHigh) ?: colors.surfaceContainerHigh
            val text = p?.rgb("text", colors.onSurface) ?: colors.onSurface
            val accent = p?.rgb("accent", colors.primary) ?: colors.primary
            val clip = androidx.compose.ui.graphics.Path().apply {
                if (halves.size == 1) addRect(androidx.compose.ui.geometry.Rect(Offset.Zero, size))
                else if (index == 0) { moveTo(0f, 0f); lineTo(size.width, 0f); lineTo(0f, size.height); close() }
                else { moveTo(size.width, 0f); lineTo(size.width, size.height); lineTo(0f, size.height); close() }
            }
            clipPath(clip) {
                drawRect(bg)
                val u = size.height / 34f
                drawRoundRect(surface, Offset(6 * u, 6 * u), Size(size.width - 12 * u, 10 * u), CornerRadius(3 * u))
                drawRoundRect(text, Offset(9 * u, 9.5f * u), Size(18 * u, 3 * u), CornerRadius(1.5f * u))
                drawRoundRect(text.copy(alpha = 0.55f), Offset(6 * u, 21 * u), Size(24 * u, 3 * u), CornerRadius(1.5f * u))
                drawCircle(accent, 4.5f * u, Offset(size.width - 11 * u, 25 * u))
            }
        }
    }
}
