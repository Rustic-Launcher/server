// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import io.papermc.paper.ServerBuildInfo;
import java.time.Instant;
import java.util.Optional;
import java.util.OptionalInt;
import net.kyori.adventure.key.Key;

/** The Paper API's {@link ServerBuildInfo} service (read by {@code Bukkit.setServer}, bStats...). */
public final class BuildInfo implements ServerBuildInfo {
    private static final Key BRAND = Key.key("rustic", "bridge");

    @Override
    public Key brandId() {
        return BRAND;
    }

    @Override
    public boolean isBrandCompatible(Key key) {
        return BRAND.equals(key);
    }

    @Override
    public String brandName() {
        return "Rustic";
    }

    @Override
    public String minecraftVersionId() {
        return "26.3";
    }

    @Override
    public String minecraftVersionName() {
        return "26.3";
    }

    @Override
    public OptionalInt buildNumber() {
        return OptionalInt.empty();
    }

    @Override
    public Instant buildTime() {
        return Instant.EPOCH;
    }

    @Override
    public Optional<String> gitBranch() {
        return Optional.empty();
    }

    @Override
    public Optional<String> gitCommit() {
        return Optional.empty();
    }

    @Override
    public String asString(StringRepresentation representation) {
        return representation == StringRepresentation.VERSION_SIMPLE
                ? "26.3-rustic-bridge-" + Bridge.VERSION
                : "Rustic Bridge " + Bridge.VERSION + " (Paper API 26.3, on Horizon)";
    }
}
