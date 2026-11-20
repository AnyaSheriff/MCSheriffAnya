package ru.mcsa.bukkit;

import java.util.ArrayList;
import java.util.List;
import java.util.Objects;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.format.NamedTextColor;
import net.kyori.adventure.text.format.TextDecoration;
import net.kyori.adventure.text.minimessage.MiniMessage;

// Текстовые теги MiniMessage: цвета, оформление, переносы и градиент без фазы.
public final class MiniText implements MiniMessage {
    public static final MiniText INSTANCE = new MiniText();
    private static final Frame ROOT = new Frame("", -1, 0, 0, null);

    private MiniText() {}

    @Override public Component deserialize(String input) {
        Objects.requireNonNull(input);
        List<Frame> stack = new ArrayList<>();
        stack.add(ROOT);
        List<Segment> segments = new ArrayList<>();
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < input.length(); i++) {
            char current = input.charAt(i);
            if (current == '\\' && i + 1 < input.length() && (input.charAt(i + 1) == '<' || input.charAt(i + 1) == '\\')) {
                text.append(input.charAt(++i));
                continue;
            }
            int end = current == '<' ? endOfTag(input, i + 1) : -1;
            if (end < 0) {
                text.append(current);
                continue;
            }
            Frame before = stack.get(stack.size() - 1);
            String tag = input.substring(i + 1, end);
            if (!applyTag(tag, before, stack, segments, text)) text.append(input, i, end + 1);
            i = end;
        }
        flush(segments, text, stack.get(stack.size() - 1));
        List<Component> output = new ArrayList<>();
        for (Segment segment : segments) segment.render(output);
        return Text.sequence(output);
    }

    private static int endOfTag(String input, int start) {
        char quote = 0;
        for (int i = start; i < input.length(); i++) {
            char c = input.charAt(i);
            if (quote != 0) {
                if (c == '\\') i++;
                else if (c == quote) quote = 0;
            } else if (c == '\'' || c == '"') quote = c;
            else if (c == '<') return -1;
            else if (c == '>') return i;
        }
        return -1;
    }

    private static boolean applyTag(String tag, Frame before, List<Frame> stack, List<Segment> segments, StringBuilder text) {
        boolean closing = tag.startsWith("/");
        boolean solitary = tag.endsWith("/");
        if (closing) tag = tag.substring(1);
        if (solitary && !tag.isEmpty()) tag = tag.substring(0, tag.length() - 1);
        String[] args = tag.split(":", -1);
        String name = Ascii.lower(args[0]);
        if (name.isEmpty()) return false;
        if (closing) {
            if (args.length != 1 || solitary) return false;
            String key = key(name);
            for (int at = stack.size() - 1; at > 0; at--) {
                if (!stack.get(at).tag.equals(key)) continue;
                flush(segments, text, before);
                stack.subList(at, stack.size()).clear();
                return true;
            }
            return false;
        }
        if ((name.equals("br") || name.equals("newline")) && args.length == 1) {
            text.append('\n');
            return true;
        }
        if (name.equals("reset") && args.length == 1) {
            flush(segments, text, before);
            stack.subList(1, stack.size()).clear();
            return true;
        }
        Frame after = frame(before, name, args);
        if (after == null) return false;
        if (!solitary) {
            flush(segments, text, before);
            stack.add(after);
        }
        return true;
    }

    private static Frame frame(Frame before, String name, String[] args) {
        boolean negated = name.startsWith("!");
        String key = key(negated ? name.substring(1) : name);
        TextDecoration decoration = decoration(key);
        if (decoration != null && args.length <= 2) {
            String value = args.length == 2 ? argument(args[1]) : "true";
            if (!value.equals("true") && !value.equals("false")) return null;
            int bit = 1 << decoration.ordinal();
            int enabled = !negated && value.equals("true") ? before.enabled | bit : before.enabled & ~bit;
            return new Frame(key, before.rgb, before.specified | bit, enabled, before.gradient);
        }
        if (negated) return null;
        if (name.equals("gradient") && args.length > 1) {
            int[] colors = new int[args.length - 1];
            for (int i = 0; i < colors.length; i++) {
                colors[i] = color(argument(args[i + 1]));
                if (colors[i] < 0) return null;
            }
            return new Frame(name, -1, before.specified, before.enabled, new Gradient(colors));
        }
        int rgb = key.equals("color") && args.length == 2 ? color(argument(args[1])) : args.length == 1 ? color(name) : -1;
        return rgb >= 0 ? new Frame(key, rgb, before.specified, before.enabled, null) : null;
    }

    private static String key(String name) {
        switch (name) {
            case "c": case "colour": return "color";
            case "b": return "bold";
            case "i": case "em": return "italic";
            case "u": case "under": return "underlined";
            case "st": return "strikethrough";
            case "obf": return "obfuscated";
            default: return name;
        }
    }

    private static String argument(String raw) {
        if (raw.length() >= 2 && ((raw.charAt(0) == '\'' && raw.endsWith("'")) || (raw.charAt(0) == '"' && raw.endsWith("\"")))) {
            return Ascii.lower(raw.substring(1, raw.length() - 1));
        }
        return Ascii.lower(raw);
    }

    private static TextDecoration decoration(String name) {
        switch (name) {
            case "bold": return TextDecoration.BOLD;
            case "italic": return TextDecoration.ITALIC;
            case "underlined": return TextDecoration.UNDERLINED;
            case "strikethrough": return TextDecoration.STRIKETHROUGH;
            case "obfuscated": return TextDecoration.OBFUSCATED;
            default: return null;
        }
    }

    private static int color(String name) {
        if (name.length() == 7 && name.charAt(0) == '#') {
            int color = 0;
            for (int i = 1; i < 7; i++) {
                int digit = "0123456789abcdef".indexOf(name.charAt(i));
                if (digit < 0) return -1;
                color = color * 16 + digit;
            }
            return color;
        }
        NamedTextColor named = NamedTextColor.NAMES.value(name);
        return named == null ? -1 : named.value();
    }

    private static void flush(List<Segment> segments, StringBuilder text, Frame frame) {
        if (text.length() == 0) return;
        segments.add(new Segment(text.toString(), frame));
        text.setLength(0);
    }

    private static final class Frame {
        final String tag;
        final int rgb, specified, enabled;
        final Gradient gradient;

        Frame(String tag, int rgb, int specified, int enabled, Gradient gradient) {
            this.tag = tag;
            this.rgb = rgb;
            this.specified = specified;
            this.enabled = enabled;
            this.gradient = gradient;
        }
    }

    private static final class Segment {
        final String text;
        final Frame style;
        final int offset;

        Segment(String text, Frame style) {
            this.text = text;
            this.style = style;
            offset = style.gradient == null ? 0 : style.gradient.points;
            if (style.gradient != null) style.gradient.points += text.codePointCount(0, text.length());
        }

        void render(List<Component> output) {
            if (style.gradient == null) {
                output.add(Text.styled(text, style.rgb, style.specified, style.enabled));
                return;
            }
            int point = offset;
            for (int at = 0; at < text.length();) {
                int end = at + Character.charCount(text.codePointAt(at));
                output.add(Text.styled(text.substring(at, end), style.gradient.at(point++), style.specified, style.enabled));
                at = end;
            }
        }
    }

    private static final class Gradient {
        final int[] colors;
        int points;

        Gradient(int[] colors) { this.colors = colors; }

        int at(int point) {
            if (colors.length == 1 || points <= 1) return colors[0];
            double position = (double)point * (colors.length - 1) / (points - 1);
            int left = (int)position;
            if (left >= colors.length - 1) return colors[left];
            int from = colors[left], to = colors[left + 1];
            double ratio = position - left;
            return mix(from >>> 16, to >>> 16, ratio) << 16
                | mix(from >>> 8 & 255, to >>> 8 & 255, ratio) << 8
                | mix(from & 255, to & 255, ratio);
        }

        private static int mix(int from, int to, double ratio) {
            return (int)Math.round(from + (to - from) * ratio);
        }
    }
}
