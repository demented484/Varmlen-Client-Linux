package app.varmlen.client

import android.app.Activity
import android.content.Intent
import android.net.VpnService
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class ConnectArgs {
    var config: String = ""
    var socksPort: Int = 10808
    var dns: String = "1.1.1.1"
    var apps: Array<String> = arrayOf()
    var appsAllow: Boolean = false
    var logLevel: String = "warn"
}

/** Tauri bridge: the Rust `vpn_connect`/`vpn_disconnect` commands call into this
 *  on Android to drive the VpnService (with the system consent dialog). */
@TauriPlugin
class VpnPlugin(private val activity: Activity) : Plugin(activity) {
    private var pendingArgs: ConnectArgs? = null

    @Command
    fun connect(invoke: Invoke) {
        val args = invoke.parseArgs(ConnectArgs::class.java)
        val consent = VpnService.prepare(activity)
        if (consent != null) {
            // First run: ask for VPN permission, then start on the result.
            pendingArgs = args
            startActivityForResult(invoke, consent, "onConsent")
            return
        }
        startVpn(args)
        invoke.resolve()
    }

    @ActivityCallback
    fun onConsent(invoke: Invoke, result: ActivityResult) {
        val args = pendingArgs
        pendingArgs = null
        if (result.resultCode == Activity.RESULT_OK && args != null) {
            startVpn(args)
            invoke.resolve()
        } else {
            invoke.reject("VPN permission denied")
        }
    }

    @Command
    fun disconnect(invoke: Invoke) {
        val intent = Intent(activity, VarmlenVpnService::class.java)
        intent.action = VarmlenVpnService.ACTION_DISCONNECT
        activity.startService(intent)
        invoke.resolve()
    }

    @Command
    fun status(invoke: Invoke) {
        val ret = JSObject()
        ret.put("running", VarmlenVpnService.running)
        invoke.resolve(ret)
    }

    @Command
    fun readLog(invoke: Invoke) {
        val ret = JSObject()
        val f = java.io.File(activity.filesDir, VarmlenVpnService.LOG_FILE)
        ret.put("log", if (f.exists()) f.readText() else "")
        invoke.resolve(ret)
    }

    @Command
    fun clearLog(invoke: Invoke) {
        try { java.io.File(activity.filesDir, VarmlenVpnService.LOG_FILE).writeText("") } catch (_: Throwable) {}
        invoke.resolve()
    }

    /** Launchable apps (the ones a user recognises), for the split-tunnel picker. */
    @Command
    fun listApps(invoke: Invoke) {
        val pm = activity.packageManager
        val main = Intent(Intent.ACTION_MAIN, null).addCategory(Intent.CATEGORY_LAUNCHER)
        val arr = JSArray()
        val seen = HashSet<String>()
        for (ri in pm.queryIntentActivities(main, 0)) {
            val pkg = ri.activityInfo?.packageName ?: continue
            if (pkg == activity.packageName || !seen.add(pkg)) continue
            val o = JSObject()
            o.put("id", pkg)
            o.put("name", ri.loadLabel(pm).toString())
            o.put("icon", null)
            arr.put(o)
        }
        val ret = JSObject()
        ret.put("apps", arr)
        invoke.resolve(ret)
    }

    private fun startVpn(args: ConnectArgs) {
        val intent = Intent(activity, VarmlenVpnService::class.java)
        intent.action = VarmlenVpnService.ACTION_CONNECT
        intent.putExtra(VarmlenVpnService.EXTRA_CONFIG, args.config)
        intent.putExtra(VarmlenVpnService.EXTRA_SOCKS_PORT, args.socksPort)
        intent.putExtra(VarmlenVpnService.EXTRA_DNS, args.dns)
        intent.putExtra(VarmlenVpnService.EXTRA_APPS, args.apps)
        intent.putExtra(VarmlenVpnService.EXTRA_APPS_ALLOW, args.appsAllow)
        intent.putExtra(VarmlenVpnService.EXTRA_LOG_LEVEL, args.logLevel)
        // startService (not startForegroundService): we're invoked from the
        // foreground activity, so this is allowed and avoids the strict
        // "must call startForeground within 5s" crash on Android 14+.
        activity.startService(intent)
    }
}
