// Changed: Replaced default counter test with TASMail app smoke test
import 'package:flutter_test/flutter_test.dart';
import 'package:tasmail_mobile/main.dart';

void main() {
  testWidgets('TASMail app renders splash or login screen', (WidgetTester tester) async {
    await tester.pumpWidget(const TasMailApp());

    // NOTE: App should show either splash (loading) or login screen
    expect(find.text('TASMail'), findsOneWidget);
  });
}
