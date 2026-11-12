// SPDX-License-Identifier: GPL-3.0-only
package rustictest.shared;

/** A "shaded library" class: RusticCommands and Welcome each bundle their own copy. */
public final class Lib {
    private Lib() {}

    public static final String VERSION = "commands-copy";
}
