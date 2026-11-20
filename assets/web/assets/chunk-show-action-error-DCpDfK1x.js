import{_ as e}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{t}from"./chunk-eventbus-Ca-q0d_H.js";import{ct as n}from"./chunk-http-55heppqR.js";import{I as r,i as ee}from"./chunk-gql-client-BC6HYZRk.js";import{At as i,Ft as a,It as o,Lt as s,Mt as c,Nt as l,Pt as u,Rt as d,jt as f,zt as p}from"./chunk-query-x-brNCkl.js";function te(t,r){let i=e(!1),a=[],o=r?[r]:[];async function s(e){i.value=!0;try{let r=await ee(t.document,e,{dedupe:!1,...t.origin?{origin:!0}:{}});if(r.errors?.length)throw n.fromGraphql(r.errors[0]);for(let e of a)e(r);return r}catch(e){if(e instanceof n&&e.canceled)return;for(let t of o)t(e);return}finally{i.value=!1}}function c(e){return a.push(e),{off:()=>{let t=a.indexOf(e);t>=0&&a.splice(t,1)}}}function l(e){return o.push(e),{off:()=>{let t=o.indexOf(e);t>=0&&o.splice(t,1)}}}return{mutate:s,loading:i,onDone:c,onError:l}}async function m(e,t){return await e(t)!=null}var h=`
  mutation {
    clearAppLogs
  }
`,g=`
  mutation setPref($key: String!, $value: JSON!) {
    setPref(key: $key, value: $value)
  }
`,_=`
  mutation updateDeviceName($name: String!) {
    updateDeviceName(name: $name)
  }
`,v=`
  mutation forwardChatItems($id: ID!, $targets: [String!]!, $note: String) {
    forwardChatItems(id: $id, targets: $targets, note: $note) {
      ...ChatItemFragment
    }
  }
  ${l}
`,y=`
  mutation sendChatItems($targets: [String!]!, $content: ChatContentInput!) {
    sendChatItems(targets: $targets, content: $content) {
      ...ChatItemFragment
    }
  }
  ${l}
`,b=`
  mutation deleteChatItem($id: ID!) {
    deleteChatItem(id: $id)
  }
`,x=`
  mutation deleteChatItems($query: String!) {
    deleteChatItems(query: $query) {
      affectedCount
    }
  }
`,S=`
  mutation retryChatItem($id: ID!) {
    retryChatItem(id: $id) {
      ...ChatItemFragment
    }
  }
  ${l}
`,C=`
  mutation createChatChannel($name: String!) {
    createChatChannel(name: $name) {
      ...ChatChannelFragment
    }
  }
  ${c}
`,w=`
  mutation updateChatChannel($id: ID!, $name: String!) {
    updateChatChannel(id: $id, name: $name) {
      ...ChatChannelFragment
    }
  }
  ${c}
`,T=`
  mutation deleteChatChannel($id: ID!) {
    deleteChatChannel(id: $id)
  }
`,E=`
  mutation deletePeer($peerId: ID!) {
    deletePeer(peerId: $peerId)
  }
`,D=`
  mutation unpairPeer($peerId: ID!) {
    unpairPeer(peerId: $peerId)
  }
`,O=`
  mutation leaveChatChannel($id: ID!) {
    leaveChatChannel(id: $id)
  }
`,k=`
  mutation addChatChannelMember($id: ID!, $peerId: ID!) {
    addChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${c}
`,A=`
  mutation removeChatChannelMember($id: ID!, $peerId: ID!) {
    removeChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${c}
`,j=`
  mutation acceptChatChannelInvite($id: ID!) {
    acceptChatChannelInvite(id: $id)
  }
`,M=`
  mutation declineChatChannelInvite($id: ID!) {
    declineChatChannelInvite(id: $id)
  }
`,N=`
  mutation createDir($path: String!) {
    createDir(path: $path) {
      ...FileFragment
    }
  }
  ${s}
`,P=`
  mutation writeTextFile($path: String!, $content: String!, $overwrite: Boolean!) {
    writeTextFile(path: $path, content: $content, overwrite: $overwrite) {
      ...FileFragment
    }
  }
  ${s}
`,F=`
  mutation renameFile($path: String!, $name: String!) {
    renameFile(path: $path, name: $name)
  }
`,I=`
  mutation copyFileToDirectory($src: String!, $directory: String!, $name: String!) {
    copyFileToDirectory(src: $src, directory: $directory, name: $name)
  }
`,L=`
  mutation createFileTask($type: FileTaskType!, $title: String!, $ops: [FileTaskOpInput!]!) {
    createFileTask(type: $type, title: $title, ops: $ops, waitForCompletion: true) {
      status
      error
    }
  }
`,R=`
  mutation recordMediaPlay($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    recordMediaPlay(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,z=`
  mutation saveMediaPlayProgress($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    saveMediaPlayProgress(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,B=`
  mutation selectAudio($mediaId: ID!) {
    selectAudio(mediaId: $mediaId) {
      mediaId
    }
  }
`,V=`
  mutation enqueueAndSelectAudio($mediaId: ID!, $query: String!) {
    addAudioToQueue(query: $query)
    selectAudio(mediaId: $mediaId) {
      mediaId
    }
  }
`,H=`
  mutation updateAudioPlayMode($mode: MediaPlayMode!) {
    updateAudioPlayMode(mode: $mode)
  }
`,U=`
  mutation removeAudioFromQueue($mediaId: ID!) {
    removeAudioFromQueue(mediaId: $mediaId)
  }
`,W=`
  mutation addAudioToQueue($query: String!) {
    addAudioToQueue(query: $query)
  }
`,G=`
  mutation clearAudioQueue {
    clearAudioQueue
  }
`,K=`
  mutation reorderAudioQueue($mediaIds: [ID!]!) {
    reorderAudioQueue(mediaIds: $mediaIds)
  }
`,q=`
  mutation deleteMediaItems($type: MediaDataType!, $query: String!) {
    deleteMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,J=`
  mutation trashMediaItems($type: MediaDataType!, $query: String!) {
    trashMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,Y=`
  mutation restoreMediaItems($type: MediaDataType!, $query: String!) {
    restoreMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,X=`
  mutation moveMediaItems($type: MediaDataType!, $query: String!, $destDir: String!) {
    moveMediaItems(type: $type, query: $query, destDir: $destDir) {
      affectedCount
    }
  }
`,Z=`
  mutation trashSms($query: String!) {
    trashSms(query: $query) {
      affectedCount
    }
  }
`,Q=`
  mutation restoreSms($query: String!) {
    restoreSms(query: $query) {
      affectedCount
    }
  }
`,ne=`
  mutation deleteSms($query: String!) {
    deleteSms(query: $query) {
      affectedCount
    }
  }
`,re=`
  mutation removeFromTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    removeFromTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,ie=`
  mutation addToTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    addToTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,ae=`
  mutation updateTagRelations($type: DataType!, $keys: [String!]!, $addTagIds: [ID!]!, $removeTagIds: [ID!]!) {
    updateTagRelations(type: $type, keys: $keys, addTagIds: $addTagIds, removeTagIds: $removeTagIds)
  }
`,oe=`
  mutation createTag($type: DataType!, $name: String!) {
    createTag(type: $type, name: $name) {
      ...TagFragment
    }
  }
  ${p}
`,se=`
  mutation updateTag($id: ID!, $name: String!) {
    updateTag(id: $id, name: $name) {
      ...TagFragment
    }
  }
  ${p}
`,ce=`
  mutation deleteTag($id: ID!) {
    deleteTag(id: $id)
  }
`,le=`
  mutation addFavoriteFolder($rootPath: String!, $fullPath: String!) {
    addFavoriteFolder(rootPath: $rootPath, fullPath: $fullPath) {
      rootPath
      fullPath
    }
  }
`,ue=`
  mutation removeFavoriteFolder($fullPath: String!) {
    removeFavoriteFolder(fullPath: $fullPath) {
      rootPath
      fullPath
      alias
    }
  }
`,de=`
  mutation setMountAlias($id: ID!, $alias: String!) {
    setMountAlias(id: $id, alias: $alias)
  }
`,fe=`
  mutation setFavoriteFolderAlias($fullPath: String!, $alias: String!) {
    setFavoriteFolderAlias(fullPath: $fullPath, alias: $alias) {
      rootPath
      fullPath
      alias
    }
  }
`,pe=`
  mutation createNote($input: NoteInput!) {
    createNote(input: $input) {
      ...NoteFragment
    }
  }
  ${d}
`,me=`
  mutation updateNote($id: ID!, $input: NoteInput!) {
    updateNote(id: $id, input: $input) {
      ...NoteFragment
    }
  }
  ${d}
`,he=`
  mutation deleteNotes($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,ge=`
  mutation trashNotes($query: String!) {
    trashNotes(query: $query) {
      affectedCount
    }
  }
`,_e=`
  mutation restoreNotes($query: String!) {
    restoreNotes(query: $query) {
      affectedCount
    }
  }
`,ve=`
  mutation deleteFeedEntries($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,ye=`
  mutation deleteCalls($query: String!) {
    deleteCalls(query: $query) {
      affectedCount
    }
  }
`,be=`
  mutation deleteContacts($query: String!) {
    deleteContacts(query: $query) {
      affectedCount
    }
  }
`,xe=`
  mutation createFeed($url: String!, $fetchContent: Boolean!) {
    createFeed(url: $url, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${o}
`,Se=`
  mutation importFeeds($content: String!) {
    importFeeds(content: $content)
  }
`,Ce=`
  mutation relaunchApp {
    relaunchApp
  }
`,we=`
  mutation openAccessibilitySettings {
    openAccessibilitySettings
  }
`,Te=`
  mutation requestAccess($feature: AccessFeature!) {
    requestAccess(feature: $feature)
  }
`,Ee=`
  mutation deleteFeed($id: ID!) {
    deleteFeed(id: $id)
  }
`,De=`
  mutation updateFeed($id: ID!, $name: String!, $fetchContent: Boolean!) {
    updateFeed(id: $id, name: $name, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${o}
`,Oe=`
  mutation syncFeeds($id: ID) {
    syncFeeds(id: $id)
  }
`,ke=`
  mutation syncFeedEntryContent($id: ID!) {
    syncFeedEntryContent(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${o}
  ${a}
`,Ae=`
  mutation call($number: String!, $showDialer: Boolean!) {
    call(number: $number, showDialer: $showDialer)
  }
`,je=`
  mutation setClipboard($text: String!) {
    setClipboard(text: $text)
  }
`,Me=`
  mutation sendSms($number: String!, $body: String!, $attachmentPaths: [String!]! = [], $subscriptionId: Int, $requestId: String!) {
    sendSms(number: $number, body: $body, attachmentPaths: $attachmentPaths, subscriptionId: $subscriptionId, requestId: $requestId) {
      requestId
      requiresConfirmation
    }
  }
`,Ne=`
  mutation archiveSmsConversation($id: String!) {
    archiveSmsConversation(id: $id)
  }
`,Pe=`
  mutation unarchiveSmsConversation($id: String!) {
    unarchiveSmsConversation(id: $id)
  }
`,Fe=`
  mutation uninstallPackages($id: ID!) {
    uninstallPackages(ids: [$id])
  }
`,Ie=`
  mutation installPackage($path: String!) {
    installPackage(path: $path) {
      id
      updatedAt
      isNew
    }
  }
`,Le=`
  mutation startScreenMirror($audio: Boolean!) {
    startScreenMirror(audio: $audio)
  }
`,Re=`
  mutation requestScreenMirrorAudio {
    requestScreenMirrorAudio
  }
`,ze=`
  mutation stopScreenMirror {
    stopScreenMirror
  }
`,Be=`
  mutation setTempValue($key: String!, $value: String!) {
    setTempValue(key: $key, value: $value) {
      key
      value
    }
  }
`,Ve=`
  mutation deleteNotifications($ids: [ID!]!) {
    deleteNotifications(ids: $ids) {
      affectedCount
    }
  }
`,He=`
  mutation replyNotification($id: ID!, $actionIndex: Int!, $text: String!) {
    replyNotification(id: $id, actionIndex: $actionIndex, text: $text)
  }
`,Ue=`
  mutation deleteClipboardItems($query: String!) {
    deleteClipboardItems(query: $query) {
      affectedCount
    }
  }
`,We=`
  mutation updateScreenMirrorQuality($mode: ScreenMirrorMode!) {
    updateScreenMirrorQuality(mode: $mode)
  }
`,Ge=`
  mutation saveFeedEntriesToNotes($query: String!) {
    saveFeedEntriesToNotes(query: $query)
  }
`,Ke=`
  mutation mergeChunks($fileId: String!, $totalChunks: Int!, $path: String!, $replace: Boolean!, $totalSize: Long!) {
    mergeChunks(fileId: $fileId, totalChunks: $totalChunks, path: $path, replace: $replace, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,qe=`
  mutation mergeAppFileChunks($fileId: String!, $totalChunks: Int!, $fileName: String!, $totalSize: Long!) {
    mergeAppFileChunks(fileId: $fileId, totalChunks: $totalChunks, fileName: $fileName, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Je=`
  mutation deleteChunks($fileId: String!) {
    deleteChunks(fileId: $fileId)
  }
`,Ye=`
  mutation startPomodoro($durationSec: Int!) {
    startPomodoro(durationSec: $durationSec) { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Xe=`
  mutation stopPomodoro {
    stopPomodoro { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Ze=`
  mutation pausePomodoro {
    pausePomodoro { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Qe=`mutation logout { logout }`,$e=`
  mutation importBookmarks($content: String!) {
    importBookmarks(content: $content) { affectedCount }
  }
`,et=`
  mutation addBookmarks($urls: [String!]!, $groupId: ID!) {
    addBookmarks(urls: $urls, groupId: $groupId) {
      ...BookmarkFragment
    }
  }
  ${i}
`,tt=`
  mutation updateBookmark($id: ID!, $input: BookmarkInput!) {
    updateBookmark(id: $id, input: $input) {
      ...BookmarkFragment
    }
  }
  ${i}
`,nt=`
  mutation deleteBookmarks($ids: [ID!]!) {
    deleteBookmarks(ids: $ids) {
      affectedCount
    }
  }
`,rt=`
  mutation recordBookmarkClick($id: ID!) {
    recordBookmarkClick(id: $id)
  }
`,it=`
  mutation createBookmarkGroup($name: String!) {
    createBookmarkGroup(name: $name) {
      ...BookmarkGroupFragment
    }
  }
  ${f}
`,at=`
  mutation updateBookmarkGroup($id: ID!, $name: String!, $collapsed: Boolean!, $sortOrder: Int!) {
    updateBookmarkGroup(id: $id, name: $name, collapsed: $collapsed, sortOrder: $sortOrder) {
      ...BookmarkGroupFragment
    }
  }
  ${f}
`,ot=`
  mutation deleteBookmarkGroup($id: ID!) {
    deleteBookmarkGroup(id: $id)
  }
`,st=`
  mutation deleteFiles($paths: [String!]!) {
    deleteFiles(paths: $paths) {
      affectedCount
    }
  }
`,ct=`
  mutation { importImageSearchModel }
`,lt=`
  mutation { enableImageSearch }
`,ut=`
  mutation { disableImageSearch }
`,dt=`
  mutation { cancelImageModelDownload }
`,ft=`
  mutation startImageIndex($force: Boolean) {
    startImageIndex(force: $force)
  }
`,pt=`
  mutation { cancelImageIndex }
`,mt=`
  mutation createContact($input: ContactInput!) {
    createContact(input: $input) {
      ...ContactFragment
    }
  }
  ${u}
`,ht=`
  mutation updateContact($id: ID!, $input: ContactInput!) {
    updateContact(id: $id, input: $input) {
      ...ContactFragment
    }
  }
  ${u}
`,gt=`
  mutation DeleteNote($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,_t=`
  mutation deleteFeedEntry($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,vt=`
  mutation removePref($key: String!) {
    removePref(key: $key)
  }
`,yt=`
  mutation DeleteDbTableRows($table: String!, $ids: [String!]!) {
    deleteDbTableRows(table: $table, ids: $ids)
  }
`,$=`
  mutation StartDiscovery {
    startDiscovery
  }
`,bt=`
  mutation StopDiscovery {
    stopDiscovery
  }
`,xt=`
  mutation PairDevice($peerId: ID!, $input: PairPeerInput!) {
    pairPeer(peerId: $peerId, input: $input)
  }
`,St=`
  mutation cancelPairing($peerId: ID!) {
    cancelPairing(peerId: $peerId)
  }
`,Ct=`
  mutation respondToPairing($peerId: ID!, $accepted: Boolean!) {
    respondToPairing(peerId: $peerId, accepted: $accepted)
  }
`,wt=`
  mutation createDownload($input: DownloadInput!) {
    createDownload(input: $input) { id }
  }
`,Tt=`
  mutation pauseDownload($id: ID!) { pauseDownload(id: $id) }
`,Et=`
  mutation resumeDownload($id: ID!) { resumeDownload(id: $id) }
`,Dt=`
  mutation retryDownload($id: ID!) { retryDownload(id: $id) }
`,Ot=`
  mutation pauseFileIndex($volumeId: ID) { pauseFileIndex(volumeId: $volumeId) }
`,kt=`
  mutation resumeFileIndex($volumeId: ID) { resumeFileIndex(volumeId: $volumeId) }
`,At=`
  mutation stopFileIndex($volumeId: ID) { stopFileIndex(volumeId: $volumeId) }
`,jt=`
  mutation formatDisk($path: String!) {
    formatDisk(path: $path)
  }
`;function Mt(e){t.emit(`toast`,e instanceof n?e.message:r.NetworkError)}export{Se as $,ke as $t,b as A,Et as At,q as B,je as Bt,pe as C,He as Ct,nt as D,Y as Dt,ot as E,Ct as Et,yt as F,Ge as Ft,ne as G,$ as Gt,he as H,de as Ht,ve as I,z as It,lt as J,Le as Jt,ce as K,ft as Kt,_t as L,B as Lt,Je as M,S as Mt,Ue as N,Dt as Nt,ye as O,_e as Ot,be as P,m as Pt,$e as Q,ze as Qt,Ee as R,y as Rt,L as S,K as St,M as T,Re as Tt,Ve as U,g as Ut,gt as V,fe as Vt,E as W,Be as Wt,jt as X,At as Xt,V as Y,bt as Yt,v as Z,Xe as Zt,C as _,ae as _n,A as _t,k as a,Fe as an,qe as at,wt as b,vt as bt,Ne as c,tt as cn,we as ct,dt as d,ht as dn,Ot as dt,Oe as en,ct as et,St as f,_ as fn,Ze as ft,it as g,se as gn,U as gt,I as h,We as hn,Ce as ht,et as i,Pe as in,Qe as it,x as j,kt as jt,T as k,Q as kt,Ae as l,at as ln,xt as lt,G as m,me as mn,R as mt,j as n,ge as nn,Ie as nt,le as o,D as on,Ke as ot,h as p,De as pn,rt as pt,ut as q,Ye as qt,W as r,Z as rn,O as rt,ie as s,H as sn,X as st,Mt as t,J as tn,te as tt,pt as u,w as un,Tt as ut,mt as v,P as vn,ue as vt,oe as w,Te as wt,xe as x,F as xt,N as y,re as yt,st as z,Me as zt};