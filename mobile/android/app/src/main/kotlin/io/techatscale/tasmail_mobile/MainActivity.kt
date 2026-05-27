package io.techatscale.tasmail_mobile

// Changed: Switched from FlutterActivity to FlutterFragmentActivity for TMAIL-142
// local_auth requires FragmentActivity to host the biometric prompt fragment.
import io.flutter.embedding.android.FlutterFragmentActivity

class MainActivity : FlutterFragmentActivity()
