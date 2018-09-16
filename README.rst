MP4
=======

:Date: 09/16 2018

.. contents::


MP4 Track
--------------

支持的媒体类型:

*   Audio
*   Video

音频媒体支持的格式:

*   MP3
*   AAC
*   FLAC
*   Opus
*   LPCM
*   ALAC
*   EncryptedAudio

视频媒体支持的格式:

*   H264
*   MP4V
*   VP10
*   VP9
*   VP8
*   EncryptedVideo


MP4 Sample 中的 H.264 流
----------------------------

当 `MP4` 容器当中的 `Track` 媒体格式 是 `H264` 时，
MP4 当中的 `Sample` 资料是由连续的 `AU` ( Access Unit ) 组成的。


H.264 Stream Format
------------------------

*    Annex-B Byte Stream
*    AVC

在 MP4 当中， H264 视频流采用的是 `AVC` 的编码方式。

*Annex-B Byte Stream*:

`H.264` 编码时，在每个 `NAL` 前添加起始码 `0x00_00_01`，解码器在码流中检测到起始码，当前 `NAL` 结束。
为了防止 `NAL` 内部出现 `0x00_00_01` 的数据，`H.264` 又提出“防止竞争 emulation prevention”机制，在
编码完一个 `NAL` 时，如果检测出有连续两个 `0x00` 字节，就在后面插入一个 `0x03`。
当解码器在 `NAL` 内部检测到 `0x00_00_03` 的数据，就把 `0x03` 抛弃，恢复原始数据。

::
   0x000000  >>>>>>  0x00000300
   0x000001  >>>>>>  0x00000301
   0x000002  >>>>>>  0x00000302
   0x000003  >>>>>>  0x00000303


术语介绍
---------

*   `H264` 是视频格式，它由一连串的 `AU` 元素按照解码顺序排列 (Coded video sequence)。
*   `AU` 的全称是 `Access Unit` , 它由 一连串的 `NAL` 组成。
*   `Sample` 是 MP4 格式当中的一个最小媒体资源，它几乎等价于我们平常所说的 `视频帧` 或者 `Access Unit`。


容易疑惑的点
--------------

*   `NALUs` 不携带时间信息，时间信息在 `AUs` 上。
*   One MP4 H264 Video Sample = One AU(Access Unit) = one or more NALU


参考
--------

*   `H.264 Specs <http://www.itu.int/rec/T-REC-H.264/en>`_ , Advanced video coding for generic audiovisual services
*   `StackoverFlow: h264 inside AVI, MP4 and “Raw” h264 streams. Different format of NAL units (or ffmpeg bug) <https://stackoverflow.com/questions/46601724/h264-inside-avi-mp4-and-raw-h264-streams-different-format-of-nal-units-or-f>`_
*   `Blog: Introduction to H.264: (1) NAL Unit <https://yumichan.net/video-processing/video-compression/introduction-to-h264-nal-unit/>`_

