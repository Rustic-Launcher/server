// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import io.papermc.paper.plugin.configuration.PluginMeta;
import io.papermc.paper.plugin.provider.classloader.ConfiguredPluginClassLoader;
import io.papermc.paper.plugin.provider.classloader.PluginClassLoaderGroup;
import java.io.IOException;
import java.io.InputStream;
import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassHierarchyResolver;
import java.lang.classfile.ClassModel;
import java.lang.classfile.ClassTransform;
import java.lang.classfile.constantpool.MemberRefEntry;
import java.lang.classfile.constantpool.PoolEntry;
import java.lang.classfile.instruction.InvokeInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.MethodTypeDesc;
import java.net.URL;
import java.net.URLClassLoader;
import java.security.CodeSource;
import java.security.cert.Certificate;
import org.bukkit.plugin.java.JavaPlugin;

/**
 * One class loader per plugin jar. Parent-first (the JDK, the bridge, the Paper API and its
 * libraries), then the jar, then the classes of other loaded plugins (Bukkit lets a plugin use its
 * dependencies' classes). A plugin's shaded copies of a library are invisible to other plugins.
 *
 * <p>Classes are defined from the jar's bytes after one rewrite (JDK 25 class-file API, no agent):
 * calls to {@code System.exit}, {@code Runtime.exit} and {@code Runtime.halt} go to {@link Guard},
 * which refuses them (there is no SecurityManager in modern Java to stop a plugin ending the JVM).
 */
final class PluginClassLoader extends URLClassLoader implements ConfiguredPluginClassLoader {
    static {
        ClassLoader.registerAsParallelCapable();
    }

    private final Bridge bridge;
    private final LoadedPlugin lp;
    private final CodeSource codeSource;

    PluginClassLoader(Bridge bridge, LoadedPlugin lp) throws IOException {
        super("bukkit:" + lp.name(), new URL[] {lp.file.toURI().toURL()}, Bridge.class.getClassLoader());
        this.bridge = bridge;
        this.lp = lp;
        this.codeSource = new CodeSource(lp.file.toURI().toURL(), (Certificate[]) null);
    }

    @Override
    protected Class<?> loadClass(String name, boolean resolve) throws ClassNotFoundException {
        return loadClass(name, resolve, true, true);
    }

    @Override
    public Class<?> loadClass(String name, boolean resolve, boolean checkGlobal, boolean checkLibraries)
            throws ClassNotFoundException {
        try {
            return super.loadClass(name, resolve);
        } catch (ClassNotFoundException e) {
            if (!checkGlobal) throw e;
            for (LoadedPlugin other : bridge.plugins.values()) {
                if (other == lp || other.loader == null) continue;
                Class<?> c = other.loader.findLocal(name);
                if (c != null) return c;
            }
            throw e;
        }
    }

    /** A class of this jar only (no delegation), or null. */
    Class<?> findLocal(String name) {
        synchronized (getClassLoadingLock(name)) {
            Class<?> c = findLoadedClass(name);
            if (c != null) return c;
            try {
                return findClass(name);
            } catch (ClassNotFoundException e) {
                return null;
            }
        }
    }

    @Override
    protected Class<?> findClass(String name) throws ClassNotFoundException {
        String path = name.replace('.', '/') + ".class";
        URL url = findResource(path);
        if (url == null) throw new ClassNotFoundException(name);
        byte[] bytes;
        try (InputStream in = url.openStream()) {
            bytes = in.readAllBytes();
        } catch (IOException e) {
            throw new ClassNotFoundException(name, e);
        }
        bytes = guard(name, bytes);
        int dot = name.lastIndexOf('.');
        if (dot > 0) {
            String pkg = name.substring(0, dot);
            if (getDefinedPackage(pkg) == null) {
                try {
                    definePackage(pkg, null, null, null, null, null, null, null);
                } catch (IllegalArgumentException ignored) {
                    // defined concurrently
                }
            }
        }
        return defineClass(name, bytes, 0, bytes.length, codeSource);
    }

    // ------------------------------------------------------------------ System.exit guard

    private static final ClassDesc GUARD = ClassDesc.of("rustic.bridge.Guard");

    private static boolean isExit(String owner, String method) {
        return (owner.equals("java/lang/System") && method.equals("exit"))
                || (owner.equals("java/lang/Runtime") && (method.equals("exit") || method.equals("halt")));
    }

    private byte[] guard(String name, byte[] bytes) {
        ClassModel cm;
        try {
            cm = ClassFile.of().parse(bytes);
        } catch (IllegalArgumentException e) {
            return bytes; // a class file version the JDK does not parse: define as is
        }
        boolean hit = false;
        for (PoolEntry pe : cm.constantPool()) {
            if (pe instanceof MemberRefEntry m
                    && isExit(m.owner().asInternalName(), m.name().stringValue())) {
                hit = true;
                break;
            }
        }
        if (!hit) return bytes;
        try {
            ClassFile cf = ClassFile.of(ClassFile.ClassHierarchyResolverOption.of(
                    ClassHierarchyResolver.ofResourceParsing(this).orElse(ClassHierarchyResolver.defaultResolver())));
            byte[] out = cf.transformClass(cm, ClassTransform.transformingMethodBodies((cb, ce) -> {
                if (ce instanceof InvokeInstruction ii
                        && isExit(ii.owner().asInternalName(), ii.name().stringValue())) {
                    String owner = ii.owner().asInternalName();
                    if (owner.equals("java/lang/System")) {
                        cb.invokestatic(GUARD, "exit", MethodTypeDesc.ofDescriptor("(I)V"));
                    } else if (ii.name().stringValue().equals("halt")) {
                        cb.invokestatic(GUARD, "halt", MethodTypeDesc.ofDescriptor("(Ljava/lang/Runtime;I)V"));
                    } else {
                        cb.invokestatic(GUARD, "runtimeExit", MethodTypeDesc.ofDescriptor("(Ljava/lang/Runtime;I)V"));
                    }
                } else {
                    cb.with(ce);
                }
            }));
            BridgeLog.info(lp.name(), "guarded System.exit/Runtime.exit/halt calls in " + name);
            return out;
        } catch (RuntimeException e) {
            BridgeLog.warn(lp.name(), "could not guard exit calls in " + name + ": " + e);
            return bytes;
        }
    }

    // ------------------------------------------------------------------ ConfiguredPluginClassLoader

    @Override
    public PluginMeta getConfiguration() {
        return lp.description;
    }

    @Override
    public void init(JavaPlugin plugin) {
        plugin.init(bridge.server, lp.description, lp.dataFolder, lp.file, this, lp.description,
                BridgeLog.pluginLogger(lp.description.getPrefix() != null ? lp.description.getPrefix() : lp.name()));
    }

    @Override
    public JavaPlugin getPlugin() {
        return lp.plugin;
    }

    @Override
    public PluginClassLoaderGroup getGroup() {
        return null;
    }

    @Override
    public void close() {
        try {
            super.close();
        } catch (IOException ignored) {
            // the jar stays open until the JVM ends
        }
    }
}
