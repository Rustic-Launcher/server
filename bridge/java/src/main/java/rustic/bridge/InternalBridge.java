// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import com.destroystokyo.paper.SkinParts;
import io.papermc.paper.InternalAPIBridge;
import io.papermc.paper.command.brigadier.CommandSourceStack;
import io.papermc.paper.datacomponent.item.ResolvableProfile;
import io.papermc.paper.entity.poi.PoiType;
import io.papermc.paper.persistence.PersistentDataKey;
import io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager;
import io.papermc.paper.world.damagesource.CombatEntry;
import io.papermc.paper.world.damagesource.FallLocationType;
import java.lang.reflect.Method;
import java.util.Set;
import java.util.function.BooleanSupplier;
import java.util.function.Function;
import java.util.function.Predicate;
import net.kyori.adventure.key.Key;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.flattener.ComponentFlattener;
import org.bukkit.GameRule;
import org.bukkit.NamespacedKey;
import org.bukkit.Statistic;
import org.bukkit.attribute.Attributable;
import org.bukkit.block.Biome;
import org.bukkit.command.CommandSender;
import org.bukkit.damage.DamageEffect;
import org.bukkit.damage.DamageSource;
import org.bukkit.damage.DamageType;
import org.bukkit.enchantments.EnchantmentOffer;
import org.bukkit.entity.Entity;
import org.bukkit.entity.EntityType;
import org.bukkit.entity.LivingEntity;
import org.bukkit.entity.Pose;
import org.bukkit.entity.SpawnCategory;
import org.bukkit.inventory.ItemStack;
import org.bukkit.persistence.PersistentDataType;
import org.bukkit.plugin.Plugin;
import org.bukkit.plugin.java.JavaPlugin;

/**
 * The Paper API's service for server-provided helpers ({@link InternalAPIBridge}, found with
 * {@code ServiceLoader}). {@code JavaPlugin}'s constructor needs a lifecycle event manager from
 * it; Rustic has no Brigadier lifecycle, so registrations are accepted and logged as unsupported.
 * Everything else here is server internals the bridge does not provide.
 */
public final class InternalBridge implements InternalAPIBridge {
    private static UnsupportedOperationException no(String m) {
        return new UnsupportedOperationException("Rustic Bridge: InternalAPIBridge." + m + " not supported yet");
    }

    @Override
    public LifecycleEventManager<Plugin> createPluginLifecycleEventManager(JavaPlugin plugin, BooleanSupplier allowed) {
        @SuppressWarnings("unchecked")
        LifecycleEventManager<Plugin> m = new ApiProxy(LifecycleEventManager.class) {
            @Override
            protected Object handle(Object proxy, Method mm, String n, Object[] a) {
                if (n.equals("registerEventHandler")) {
                    BridgeLog.warn(plugin.getName(), "Paper lifecycle events (Brigadier commands, registries) are not "
                            + "supported by the Rustic Bridge; the registration is ignored");
                    return null;
                }
                return UNHANDLED;
            }
        }.create(LifecycleEventManager.class);
        return m;
    }

    @Override
    public Biome constructLegacyCustomBiome() {
        throw no("constructLegacyCustomBiome");
    }

    @Override
    public CombatEntry createCombatEntry(LivingEntity entity, DamageSource source, float damage) {
        throw no("createCombatEntry");
    }

    @Override
    public CombatEntry createCombatEntry(DamageSource source, float damage, FallLocationType fall, float fallDistance) {
        throw no("createCombatEntry");
    }

    @Override
    public Predicate<CommandSourceStack> restricted(Predicate<CommandSourceStack> predicate, boolean restricted) {
        return predicate;
    }

    @Override
    public ResolvableProfile defaultMannequinProfile() {
        throw no("defaultMannequinProfile");
    }

    @Override
    public SkinParts.Mutable allSkinParts() {
        throw no("allSkinParts");
    }

    @Override
    public Component defaultMannequinDescription() {
        return Component.empty();
    }

    @Override
    public <M, L> GameRule<L> legacyGameRuleBridge(GameRule<M> rule, Function<L, M> toModern, Function<M, L> toLegacy,
            Class<L> legacyType) {
        throw no("legacyGameRuleBridge");
    }

    @Override
    public Set<Pose> validMannequinPoses() {
        return Set.of();
    }

    @Override
    public PoiType.Occupancy createOccupancy(String name) {
        throw no("createOccupancy");
    }

    @Override
    public DamageSource.Builder createDamageSourceBuilder(DamageType type) {
        throw no("createDamageSourceBuilder");
    }

    @Override
    public DamageEffect getDamageEffect(String key) {
        throw no("getDamageEffect");
    }

    @Override
    public String getTranslationKey(EntityType type) {
        return "entity.minecraft." + type.name().toLowerCase(java.util.Locale.ROOT);
    }

    @Override
    public SpawnCategory getSpawnCategory(EntityType type) {
        return SpawnCategory.MISC;
    }

    @Override
    public ItemStack deserializeItem(byte[] data) {
        throw no("deserializeItem");
    }

    @Override
    public boolean hasDefaultEntityAttributes(NamespacedKey key) {
        return false;
    }

    @Override
    public Attributable getDefaultEntityAttributes(NamespacedKey key) {
        throw no("getDefaultEntityAttributes");
    }

    @Override
    public String getStatisticCriteriaKey(Statistic statistic) {
        throw no("getStatisticCriteriaKey");
    }

    @Override
    public ItemStack createEmptyStack() {
        throw no("createEmptyStack");
    }

    @Override
    public Component resolveWithContext(Component component, CommandSender sender, Entity entity, boolean deep) {
        return component;
    }

    @Override
    public ComponentFlattener componentFlattener() {
        return ComponentFlattener.basic();
    }

    @Override
    public <C> PersistentDataKey<C> createPersistentDataKey(Key key, PersistentDataType<?, C> type) {
        throw no("createPersistentDataKey");
    }

    @Override
    public EnchantmentOffer[] rollEnchantmentOffers(ItemStack item, int seed, int bookshelves, int slot) {
        throw no("rollEnchantmentOffers");
    }
}
