import 'package:provider/provider.dart';
import 'package:fluent_ui/fluent_ui.dart';

import '../../messages/lyric.pb.dart';
import '../../providers/responsive_providers.dart';

import 'widgets/lyric_display.dart';

class BandScreenLyricsView extends StatefulWidget {
  final int id;
  final List<LyricContentLine> lyrics;
  final int currentTimeMilliseconds;
  final List<int> activeLines;

  const BandScreenLyricsView({
    super.key,
    required this.id,
    required this.lyrics,
    required this.currentTimeMilliseconds,
    required this.activeLines,
  });

  @override
  LibraryHomeListState createState() => LibraryHomeListState();
}

class LibraryHomeListState extends State<BandScreenLyricsView> {
  @override
  Widget build(BuildContext context) {
    final r = Provider.of<ResponsiveProvider>(context);
    final isMini = r.smallerOrEqualTo(DeviceType.dock, false);

    if (isMini) {
      return RotatedBox(
        quarterTurns: 1,
        child: LyricsDisplay(
          lyrics: widget.lyrics,
          currentTimeMilliseconds: widget.currentTimeMilliseconds,
          activeLines: widget.activeLines,
        ),
      );
    }

    return LyricsDisplay(
      lyrics: widget.lyrics,
      currentTimeMilliseconds: widget.currentTimeMilliseconds,
      activeLines: widget.activeLines,
    );
  }
}