// SPDX-License-Identifier: GPL-3.0-only
package rustictest.shared;

/** A "shaded library" class: Welcome and RusticCommands each bundle their own copy. */
public final class Lib {
    private Lib() {}

    public static final String VERSION = "welcome-copy";
}
