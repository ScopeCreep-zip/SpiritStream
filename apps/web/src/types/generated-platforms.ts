// Auto-generated from data/streaming-platforms.json
// DO NOT EDIT MANUALLY
// Run 'npm run generate:platforms' to regenerate this file

/**
 * Supported streaming platforms
 * All platforms use RTMP/RTMPS with either "append" or "in_url_template" stream key placement
 */
export type Platform =
  | 'YouTube - RTMPS'
  | 'Twitch'
  | 'Kick'
  | 'Facebook Live'
  | 'LinkedIn Live'
  | 'TikTok Live'
  | 'Trovo'
  | 'Rumble'
  | 'Bilibili Live - RTMP | 哔哩哔哩直播 - RTMP'
  | 'DLive'
  | 'Streamlabs'
  | 'Restream.io'
  | 'Amazon IVS'
  | 'Nimo TV'
  | 'Steam'
  | 'Vimeo'
  | 'Twitter'
  | 'AngelThump'
  | 'Aparat'
  | 'api.video'
  | 'Bitmovin'
  | 'Bongacams'
  | 'Boomstream'
  | 'BoxCast'
  | 'Breakers.TV'
  | 'CAM4'
  | 'CamSoda'
  | 'Castr.io'
  | 'Chaturbate'
  | 'CHZZK'
  | 'Disciple Media'
  | 'Dolby OptiView Real-time'
  | 'Enchant.events'
  | 'ePlay'
  | 'Eventials'
  | 'EventLive.pro'
  | 'GoodGame.ru'
  | 'IRLToolkit'
  | 'Jio Games'
  | 'Joystick.TV'
  | 'KakaoTV'
  | 'Konduit.live'
  | 'Kuaishou Live'
  | 'Lahzenegar - StreamG | لحظه‌نگار - استریمجی'
  | 'Lightcast.com'
  | 'Livepeer Studio'
  | 'Livepush'
  | 'Loola.tv'
  | 'Lovecast'
  | 'Luzento.com - RTMP'
  | 'MasterStream.iR | مستراستریم | ری استریم و استریم همزمان'
  | 'Meridix Live Sports Platform'
  | 'Mixcloud'
  | 'Mux'
  | 'MyFreeCams'
  | 'MyLive'
  | 'nanoStream Cloud / bintu'
  | 'NFHS Network'
  | 'niconico (ニコニコ生放送)'
  | 'OnlyFans.com'
  | 'OPENREC.tv - Premium member (プレミアム会員)'
  | 'PandaTV | 판더티비'
  | 'PhoneLiveStreaming'
  | 'Picarto'
  | 'Piczel.tv'
  | 'PolyStreamer.com'
  | 'SermonAudio Cloud'
  | 'SharePlay.tv'
  | 'sheeta'
  | 'SOOP Global'
  | 'SOOP Korea'
  | 'STAGE TEN'
  | 'Streamway'
  | 'Stripchat'
  | 'Switchboard Live'
  | 'Sympla'
  | 'Uscreen'
  | 'Vaughn Live / iNSTAGIB'
  | 'Vault - by CommanderRoot'
  | 'Viloud'
  | 'Vindral'
  | 'VRCDN - Live'
  | 'Web.TV'
  | 'Whowatch (ふわっち)'
  | 'WpStream'
  | 'XLoveCam.com'
  | 'YouTube Backup - RTMPS';

/**
 * Platform configuration mapping
 * Contains display names, colors, and default server URLs
 */
export const PLATFORMS: Record<Platform, {
  displayName: string;
  abbreviation: string;
  color: string;
  textColor: string;
  defaultServer: string;
  streamKeyPlacement: 'append' | 'in_url_template';
}> = {
  'YouTube - RTMPS': {
    displayName: 'YouTube',
    abbreviation: 'YT',
    color: '#FF0000',
    textColor: '#FFFFFF',
    defaultServer: 'rtmps://a.rtmps.youtube.com:443/live2',
    streamKeyPlacement: 'append',
  },
  'Twitch': {
    displayName: 'Twitch',
    abbreviation: 'TW',
    color: '#9146FF',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://ingest.global-contribute.live-video.net/app/',
    streamKeyPlacement: 'append',
  },
  'Kick': {
    displayName: 'Kick',
    abbreviation: 'K',
    color: '#53FC18',
    textColor: '#000000',
    defaultServer: 'rtmps://fa723fc1b171.global-contribute.live-video.net/app',
    streamKeyPlacement: 'append',
  },
  'Facebook Live': {
    displayName: 'Facebook',
    abbreviation: 'FB',
    color: '#1877F2',
    textColor: '#FFFFFF',
    defaultServer: 'rtmps://rtmp-api.facebook.com:443/rtmp/',
    streamKeyPlacement: 'append',
  },
  'LinkedIn Live': {
    displayName: 'LinkedIn',
    abbreviation: 'LI',
    color: '#0A66C2',
    textColor: '#FFFFFF',
    defaultServer: 'rtmps://fa723fc1b171.global-contribute.live-video.net/app',
    streamKeyPlacement: 'append',
  },
  'TikTok Live': {
    displayName: 'TikTok',
    abbreviation: 'TT',
    color: '#000000',
    textColor: '#FFFFFF',
    defaultServer: 'rtmps://live.tiktok.com/rtmp/',
    streamKeyPlacement: 'append',
  },
  'Trovo': {
    displayName: 'Trovo',
    abbreviation: 'TR',
    color: '#1ECD97',
    textColor: '#000000',
    defaultServer: 'rtmp://livepush.trovo.live/live/',
    streamKeyPlacement: 'append',
  },
  'Rumble': {
    displayName: 'Rumble',
    abbreviation: 'R',
    color: '#85C742',
    textColor: '#000000',
    defaultServer: 'rtmp://ingest.rumble.com/app',
    streamKeyPlacement: 'append',
  },
  'Bilibili Live - RTMP | 哔哩哔哩直播 - RTMP': {
    displayName: 'Bilibili',
    abbreviation: 'BL',
    color: '#00A1D6',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://live-push.bilivideo.com/live-bvc/',
    streamKeyPlacement: 'append',
  },
  'DLive': {
    displayName: 'DLive',
    abbreviation: 'DL',
    color: '#FFD300',
    textColor: '#000000',
    defaultServer: 'rtmp://stream.dlive.tv/live',
    streamKeyPlacement: 'append',
  },
  'Streamlabs': {
    displayName: 'Streamlabs',
    abbreviation: 'SL',
    color: '#80F5D2',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.streamlabs.com/live',
    streamKeyPlacement: 'append',
  },
  'Restream.io': {
    displayName: 'Restream',
    abbreviation: 'RS',
    color: '#0081FF',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://live.restream.io/live',
    streamKeyPlacement: 'append',
  },
  'Amazon IVS': {
    displayName: 'Amazon IVS',
    abbreviation: 'AI',
    color: '#FF9900',
    textColor: '#000000',
    defaultServer: 'rtmps://hkg06.contribute.live-video.net/app',
    streamKeyPlacement: 'append',
  },
  'Nimo TV': {
    displayName: 'Nimo TV',
    abbreviation: 'NT',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://txpush.rtmp.nimo.tv/live/',
    streamKeyPlacement: 'append',
  },
  'Steam': {
    displayName: 'Steam',
    abbreviation: 'ST',
    color: '#171A21',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://ingest-rtmp.broadcast.steamcontent.com/app',
    streamKeyPlacement: 'append',
  },
  'Vimeo': {
    displayName: 'Vimeo',
    abbreviation: 'VM',
    color: '#1AB7EA',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.cloud.vimeo.com/live',
    streamKeyPlacement: 'append',
  },
  'Twitter': {
    displayName: 'Twitter',
    abbreviation: 'X',
    color: '#1DA1F2',
    textColor: '#000000',
    defaultServer: 'rtmp://ca.pscp.tv:80/x',
    streamKeyPlacement: 'append',
  },
  'AngelThump': {
    displayName: 'AngelThump',
    abbreviation: 'AT',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://ingest.angelthump.com/live',
    streamKeyPlacement: 'append',
  },
  'Aparat': {
    displayName: 'Aparat',
    abbreviation: 'AP',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.cdn.asset.aparat.com:443/event',
    streamKeyPlacement: 'append',
  },
  'api.video': {
    displayName: 'api.video',
    abbreviation: 'AV',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://broadcast.api.video/s',
    streamKeyPlacement: 'append',
  },
  'Bitmovin': {
    displayName: 'Bitmovin',
    abbreviation: 'BM',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live-input.bitmovin.com/streams',
    streamKeyPlacement: 'append',
  },
  'Bongacams': {
    displayName: 'Bongacams',
    abbreviation: 'BC',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://auto.origin.gnsbc.com:1934/live',
    streamKeyPlacement: 'append',
  },
  'Boomstream': {
    displayName: 'Boomstream',
    abbreviation: 'BS',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.boomstream.com/live',
    streamKeyPlacement: 'append',
  },
  'BoxCast': {
    displayName: 'BoxCast',
    abbreviation: 'BX',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.boxcast.com/live',
    streamKeyPlacement: 'append',
  },
  'Breakers.TV': {
    displayName: 'Breakers.TV',
    abbreviation: 'BR',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live-iad.vaughnsoft.net/live',
    streamKeyPlacement: 'append',
  },
  'CAM4': {
    displayName: 'CAM4',
    abbreviation: 'C4',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://origin.cam4.com/cam4-origin-live',
    streamKeyPlacement: 'append',
  },
  'CamSoda': {
    displayName: 'CamSoda',
    abbreviation: 'CS',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://obs-ingest-na.livemediahost.com/cam_obs',
    streamKeyPlacement: 'append',
  },
  'Castr.io': {
    displayName: 'Castr',
    abbreviation: 'CA',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://cg.castr.io/static',
    streamKeyPlacement: 'append',
  },
  'Chaturbate': {
    displayName: 'Chaturbate',
    abbreviation: 'CB',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://global.live.mmcdn.com/live-origin',
    streamKeyPlacement: 'append',
  },
  'CHZZK': {
    displayName: 'CHZZK',
    abbreviation: 'CZ',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://global-rtmp.lip2.navercorp.com:8080/relay',
    streamKeyPlacement: 'append',
  },
  'Disciple Media': {
    displayName: 'Disciple Media',
    abbreviation: 'DM',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.disciplemedia.com/b-fme',
    streamKeyPlacement: 'append',
  },
  'Dolby OptiView Real-time': {
    displayName: 'Dolby OptiView',
    abbreviation: 'DO',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmps://rtmp-auto.millicast.com:443/v2/pub',
    streamKeyPlacement: 'append',
  },
  'Enchant.events': {
    displayName: 'Enchant',
    abbreviation: 'EN',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmps://stream.enchant.cloud:443/live',
    streamKeyPlacement: 'append',
  },
  'ePlay': {
    displayName: 'ePlay',
    abbreviation: 'EP',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.eplay.link/origin',
    streamKeyPlacement: 'append',
  },
  'Eventials': {
    displayName: 'Eventials',
    abbreviation: 'EV',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://transmission.eventials.com/eventialsLiveOrigin',
    streamKeyPlacement: 'append',
  },
  'EventLive.pro': {
    displayName: 'EventLive',
    abbreviation: 'EL',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://go.eventlive.pro/live',
    streamKeyPlacement: 'append',
  },
  'GoodGame.ru': {
    displayName: 'GoodGame',
    abbreviation: 'GG',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://msk.goodgame.ru:1940/live',
    streamKeyPlacement: 'append',
  },
  'IRLToolkit': {
    displayName: 'IRLToolkit',
    abbreviation: 'IT',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmps://stream.global.irl.run/ingest',
    streamKeyPlacement: 'append',
  },
  'Jio Games': {
    displayName: 'Jio Games',
    abbreviation: 'JG',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://livepub1.api.engageapps.jio/live',
    streamKeyPlacement: 'append',
  },
  'Joystick.TV': {
    displayName: 'Joystick',
    abbreviation: 'JS',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.joystick.tv/live/',
    streamKeyPlacement: 'append',
  },
  'KakaoTV': {
    displayName: 'KakaoTV',
    abbreviation: 'KT',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.play.kakao.com/kakaotv',
    streamKeyPlacement: 'append',
  },
  'Konduit.live': {
    displayName: 'Konduit',
    abbreviation: 'KD',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.konduit.live/live',
    streamKeyPlacement: 'append',
  },
  'Kuaishou Live': {
    displayName: 'Kuaishou',
    abbreviation: 'KS',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://open-push.voip.yximgs.com/gifshow/',
    streamKeyPlacement: 'append',
  },
  'Lahzenegar - StreamG | لحظه‌نگار - استریمجی': {
    displayName: 'Lahzenegar',
    abbreviation: 'LZ',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.lahzecdn.com/pro',
    streamKeyPlacement: 'append',
  },
  'Lightcast.com': {
    displayName: 'Lightcast',
    abbreviation: 'LC',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://ingest-na1.live.lightcast.com/in',
    streamKeyPlacement: 'append',
  },
  'Livepeer Studio': {
    displayName: 'Livepeer',
    abbreviation: 'LP',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.livepeer.com/live',
    streamKeyPlacement: 'append',
  },
  'Livepush': {
    displayName: 'Livepush',
    abbreviation: 'LV',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://dc-global.livepush.io/live',
    streamKeyPlacement: 'append',
  },
  'Loola.tv': {
    displayName: 'Loola',
    abbreviation: 'LO',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.loola.tv/push',
    streamKeyPlacement: 'append',
  },
  'Lovecast': {
    displayName: 'Lovecast',
    abbreviation: 'LV',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live-a.lovecastapp.com:5222/app',
    streamKeyPlacement: 'append',
  },
  'Luzento.com - RTMP': {
    displayName: 'Luzento',
    abbreviation: 'LZ',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://ingest.luzento.com/live',
    streamKeyPlacement: 'append',
  },
  'MasterStream.iR | مستراستریم | ری استریم و استریم همزمان': {
    displayName: 'MasterStream',
    abbreviation: 'MS',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live1.masterstream.ir/live',
    streamKeyPlacement: 'append',
  },
  'Meridix Live Sports Platform': {
    displayName: 'Meridix',
    abbreviation: 'MX',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://publish.meridix.com/live',
    streamKeyPlacement: 'append',
  },
  'Mixcloud': {
    displayName: 'Mixcloud',
    abbreviation: 'MC',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.mixcloud.com/broadcast',
    streamKeyPlacement: 'append',
  },
  'Mux': {
    displayName: 'Mux',
    abbreviation: 'MX',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmps://global-live.mux.com:443/app',
    streamKeyPlacement: 'append',
  },
  'MyFreeCams': {
    displayName: 'MyFreeCams',
    abbreviation: 'MF',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://publish.myfreecams.com/NxServer',
    streamKeyPlacement: 'append',
  },
  'MyLive': {
    displayName: 'MyLive',
    abbreviation: 'ML',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://stream.mylive.in.th/live',
    streamKeyPlacement: 'append',
  },
  'nanoStream Cloud / bintu': {
    displayName: 'nanoStream',
    abbreviation: 'NS',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://bintu-stream.nanocosmos.de/live',
    streamKeyPlacement: 'append',
  },
  'NFHS Network': {
    displayName: 'NFHS',
    abbreviation: 'NH',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://video.nfhsnetwork.com/manual',
    streamKeyPlacement: 'append',
  },
  'niconico (ニコニコ生放送)': {
    displayName: 'niconico',
    abbreviation: 'NC',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://liveorigin.dlive.nicovideo.jp/live/input',
    streamKeyPlacement: 'append',
  },
  'OnlyFans.com': {
    displayName: 'OnlyFans',
    abbreviation: 'OF',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://cloudbetastreaming.onlyfans.com/live',
    streamKeyPlacement: 'append',
  },
  'OPENREC.tv - Premium member (プレミアム会員)': {
    displayName: 'OPENREC',
    abbreviation: 'OR',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://a.station.openrec.tv:1935/live1',
    streamKeyPlacement: 'append',
  },
  'PandaTV | 판더티비': {
    displayName: 'PandaTV',
    abbreviation: 'PD',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.pandalive.co.kr/app',
    streamKeyPlacement: 'append',
  },
  'PhoneLiveStreaming': {
    displayName: 'PhoneLive',
    abbreviation: 'PL',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.phonelivestreaming.com/live/',
    streamKeyPlacement: 'append',
  },
  'Picarto': {
    displayName: 'Picarto',
    abbreviation: 'PC',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.us.picarto.tv/golive',
    streamKeyPlacement: 'append',
  },
  'Piczel.tv': {
    displayName: 'Piczel',
    abbreviation: 'PZ',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://piczel.tv:1935/live',
    streamKeyPlacement: 'append',
  },
  'PolyStreamer.com': {
    displayName: 'PolyStreamer',
    abbreviation: 'PS',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.polystreamer.com/live',
    streamKeyPlacement: 'append',
  },
  'SermonAudio Cloud': {
    displayName: 'SermonAudio',
    abbreviation: 'SA',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://webcast.sermonaudio.com/sa',
    streamKeyPlacement: 'append',
  },
  'SharePlay.tv': {
    displayName: 'SharePlay',
    abbreviation: 'SP',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://stream.shareplay.tv',
    streamKeyPlacement: 'append',
  },
  'sheeta': {
    displayName: 'sheeta',
    abbreviation: 'SH',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://lsm.sheeta.com:1935/lsm',
    streamKeyPlacement: 'append',
  },
  'SOOP Global': {
    displayName: 'SOOP Global',
    abbreviation: 'SG',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://global-stream.sooplive.com/app',
    streamKeyPlacement: 'append',
  },
  'SOOP Korea': {
    displayName: 'SOOP Korea',
    abbreviation: 'SK',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://stream.sooplive.co.kr/app/',
    streamKeyPlacement: 'append',
  },
  'STAGE TEN': {
    displayName: 'STAGE TEN',
    abbreviation: 'ST',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmps://app-rtmp.stageten.tv:443/stageten',
    streamKeyPlacement: 'append',
  },
  'Streamway': {
    displayName: 'Streamway',
    abbreviation: 'SW',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://injest.streamway.in/LiveApp',
    streamKeyPlacement: 'append',
  },
  'Stripchat': {
    displayName: 'Stripchat',
    abbreviation: 'SC',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.doppiocdn.com/ext',
    streamKeyPlacement: 'append',
  },
  'Switchboard Live': {
    displayName: 'Switchboard',
    abbreviation: 'SB',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmps://live.sb.zone:443/live',
    streamKeyPlacement: 'append',
  },
  'Sympla': {
    displayName: 'Sympla',
    abbreviation: 'SY',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://rtmp.sympla.com.br:5222/app',
    streamKeyPlacement: 'append',
  },
  'Uscreen': {
    displayName: 'Uscreen',
    abbreviation: 'US',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://global-live.uscreen.app:5222/app',
    streamKeyPlacement: 'append',
  },
  'Vaughn Live / iNSTAGIB': {
    displayName: 'Vaughn Live',
    abbreviation: 'VL',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live-iad.vaughnsoft.net/live',
    streamKeyPlacement: 'append',
  },
  'Vault - by CommanderRoot': {
    displayName: 'Vault',
    abbreviation: 'VT',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://ingest-eu-central.vault.root-space.eu/app',
    streamKeyPlacement: 'append',
  },
  'Viloud': {
    displayName: 'Viloud',
    abbreviation: 'VD',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.viloud.tv:5222/app',
    streamKeyPlacement: 'append',
  },
  'Vindral': {
    displayName: 'Vindral',
    abbreviation: 'VN',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmps://rtmp.global.cdn.vindral.com/publish',
    streamKeyPlacement: 'append',
  },
  'VRCDN - Live': {
    displayName: 'VRCDN',
    abbreviation: 'VR',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://ingest.vrcdn.live/live',
    streamKeyPlacement: 'append',
  },
  'Web.TV': {
    displayName: 'Web.TV',
    abbreviation: 'WT',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live3.origins.web.tv/liveext',
    streamKeyPlacement: 'append',
  },
  'Whowatch (ふわっち)': {
    displayName: 'Whowatch',
    abbreviation: 'WW',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://live.whowatch.tv/live/',
    streamKeyPlacement: 'append',
  },
  'WpStream': {
    displayName: 'WpStream',
    abbreviation: 'WP',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://ingest.wpstream.net/golive',
    streamKeyPlacement: 'append',
  },
  'XLoveCam.com': {
    displayName: 'XLoveCam',
    abbreviation: 'XL',
    color: '#9489A8',
    textColor: '#000000',
    defaultServer: 'rtmp://nl.eu.stream.xlove.com/performer-origin',
    streamKeyPlacement: 'append',
  },
  'YouTube Backup - RTMPS': {
    displayName: 'YouTube (Backup)',
    abbreviation: 'YB',
    color: '#CC0000',
    textColor: '#FFFFFF',
    defaultServer: 'rtmp://b.rtmp.youtube.com/live2/{stream_key}?backup=1',
    streamKeyPlacement: 'in_url_template',
  }
};
