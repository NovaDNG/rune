import 'package:provider/provider.dart';
import 'package:fluent_ui/fluent_ui.dart';
import 'package:flutter_svg/flutter_svg.dart';

import '../../providers/router_path.dart';
import '../../utils/navigation/utils/navigation_backward.dart';

import '../../widgets/ax_pressure.dart';
import '../../widgets/hover_opacity.dart';

class NavigationBackButton extends StatefulWidget {
  const NavigationBackButton({
    super.key,
  });

  @override
  State<NavigationBackButton> createState() => _NavigationBackButtonState();
}

class _NavigationBackButtonState extends State<NavigationBackButton> {
  final FocusNode _focusNode = FocusNode(debugLabel: 'Back Button');

  @override
  void dispose() {
    _focusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final path = Provider.of<RouterPathProvider>(context).path;

    if (path == '/library') {
      return Container();
    }

    return AxPressure(
      child: HoverOpacity(
        child: Listener(
          onPointerUp: (_) => navigateBackwardWithPop(),
          child: FocusableActionDetector(
            focusNode: _focusNode,
            child: SvgPicture.asset(
              'assets/arrow-circle-left-solid.svg',
              width: 56,
              colorFilter: ColorFilter.mode(
                FluentTheme.of(context).inactiveColor,
                BlendMode.srcIn,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
