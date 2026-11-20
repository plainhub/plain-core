import{_ as e}from"./chunk-reactivity.esm-bundler-Dh02ET-1.js";import{t}from"./chunk-eventbus-Ca-q0d_H.js";import{ct as n}from"./chunk-http-Ko84D6-O.js";import{I as r,i as ee}from"./chunk-gql-client-CByWpbNJ.js";import{At as i,Bt as a,Ft as o,It as s,Lt as c,Mt as l,Nt as u,Pt as d,Rt as f,jt as p,zt as m}from"./chunk-query-Ckf3gnaH.js";function te(t,r){let i=e(!1),a=[],o=r?[r]:[];async function s(e){i.value=!0;try{let r=await ee(t.document,e,{dedupe:!1,...t.origin?{origin:!0}:{}});if(r.errors?.length)throw n.fromGraphql(r.errors[0]);for(let e of a)e(r);return r}catch(e){if(e instanceof n&&e.canceled)return;for(let t of o)t(e);return}finally{i.value=!1}}function c(e){return a.push(e),{off:()=>{let t=a.indexOf(e);t>=0&&a.splice(t,1)}}}function l(e){return o.push(e),{off:()=>{let t=o.indexOf(e);t>=0&&o.splice(t,1)}}}return{mutate:s,loading:i,onDone:c,onError:l}}async function h(e,t){return await e(t)!=null}var g=`
  mutation {
    clearAppLogs
  }
`,_=`
  mutation setPref($key: String!, $value: JSON!) {
    setPref(key: $key, value: $value)
  }
`,v=`
  mutation updateDeviceName($name: String!) {
    updateDeviceName(name: $name)
  }
`,y=`
  mutation forwardChatItems($id: ID!, $targets: [String!]!, $note: String) {
    forwardChatItems(id: $id, targets: $targets, note: $note) {
      ...ChatItemFragment
    }
  }
  ${d}
`,b=`
  mutation sendChatItems($targets: [String!]!, $content: ChatContentInput!) {
    sendChatItems(targets: $targets, content: $content) {
      ...ChatItemFragment
    }
  }
  ${d}
`,x=`
  mutation deleteChatItem($id: ID!) {
    deleteChatItem(id: $id)
  }
`,S=`
  mutation deleteChatItems($query: String!) {
    deleteChatItems(query: $query) {
      affectedCount
    }
  }
`,C=`
  mutation retryChatItem($id: ID!) {
    retryChatItem(id: $id) {
      ...ChatItemFragment
    }
  }
  ${d}
`,w=`
  mutation createChatChannel($name: String!) {
    createChatChannel(name: $name) {
      ...ChatChannelFragment
    }
  }
  ${u}
`,T=`
  mutation updateChatChannel($id: ID!, $name: String!) {
    updateChatChannel(id: $id, name: $name) {
      ...ChatChannelFragment
    }
  }
  ${u}
`,E=`
  mutation deleteChatChannel($id: ID!) {
    deleteChatChannel(id: $id)
  }
`,D=`
  mutation deletePeer($peerId: ID!) {
    deletePeer(peerId: $peerId)
  }
`,O=`
  mutation unpairPeer($peerId: ID!) {
    unpairPeer(peerId: $peerId)
  }
`,k=`
  mutation leaveChatChannel($id: ID!) {
    leaveChatChannel(id: $id)
  }
`,A=`
  mutation addChatChannelMember($id: ID!, $peerId: ID!) {
    addChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${u}
`,j=`
  mutation removeChatChannelMember($id: ID!, $peerId: ID!) {
    removeChatChannelMember(id: $id, peerId: $peerId) {
      ...ChatChannelFragment
    }
  }
  ${u}
`,M=`
  mutation acceptChatChannelInvite($id: ID!) {
    acceptChatChannelInvite(id: $id)
  }
`,N=`
  mutation declineChatChannelInvite($id: ID!) {
    declineChatChannelInvite(id: $id)
  }
`,P=`
  mutation createDir($path: String!) {
    createDir(path: $path) {
      ...FileFragment
    }
  }
  ${f}
`,F=`
  mutation writeTextFile($path: String!, $content: String!, $overwrite: Boolean!) {
    writeTextFile(path: $path, content: $content, overwrite: $overwrite) {
      ...FileFragment
    }
  }
  ${f}
`,I=`
  mutation renameFile($path: String!, $name: String!) {
    renameFile(path: $path, name: $name)
  }
`,L=`
  mutation copyFileToDirectory($src: String!, $directory: String!, $name: String!) {
    copyFileToDirectory(src: $src, directory: $directory, name: $name)
  }
`,R=`
  mutation createFileTask($type: FileTaskType!, $title: String!, $ops: [FileTaskOpInput!]!) {
    createFileTask(type: $type, title: $title, ops: $ops, waitForCompletion: true) {
      status
      error
    }
  }
`,z=`
  mutation recordMediaPlay($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    recordMediaPlay(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,B=`
  mutation saveMediaPlayProgress($mediaId: ID!, $type: PlaybackType!, $positionMs: Long!) {
    saveMediaPlayProgress(mediaId: $mediaId, type: $type, positionMs: $positionMs)
  }
`,V=`
  mutation selectAudio($mediaId: ID!) {
    selectAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${i}
`,H=`
  mutation enqueueAndSelectAudio($mediaId: ID!, $query: String!) {
    addAudioToQueue(query: $query)
    selectAudio(mediaId: $mediaId) {
      ...AudioItemFragment
    }
  }
  ${i}
`,U=`
  mutation updateAudioPlayMode($mode: MediaPlayMode!) {
    updateAudioPlayMode(mode: $mode)
  }
`,W=`
  mutation removeAudioFromQueue($mediaId: ID!) {
    removeAudioFromQueue(mediaId: $mediaId)
  }
`,G=`
  mutation addAudioToQueue($query: String!) {
    addAudioToQueue(query: $query)
  }
`,K=`
  mutation clearAudioQueue {
    clearAudioQueue
  }
`,q=`
  mutation reorderAudioQueue($mediaIds: [ID!]!) {
    reorderAudioQueue(mediaIds: $mediaIds)
  }
`,J=`
  mutation deleteMediaItems($type: MediaDataType!, $query: String!) {
    deleteMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,Y=`
  mutation trashMediaItems($type: MediaDataType!, $query: String!) {
    trashMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,X=`
  mutation restoreMediaItems($type: MediaDataType!, $query: String!) {
    restoreMediaItems(type: $type, query: $query) {
      affectedCount
    }
  }
`,Z=`
  mutation moveMediaItems($type: MediaDataType!, $query: String!, $destDir: String!) {
    moveMediaItems(type: $type, query: $query, destDir: $destDir) {
      affectedCount
    }
  }
`,Q=`
  mutation trashSms($query: String!) {
    trashSms(query: $query) {
      affectedCount
    }
  }
`,ne=`
  mutation restoreSms($query: String!) {
    restoreSms(query: $query) {
      affectedCount
    }
  }
`,re=`
  mutation deleteSms($query: String!) {
    deleteSms(query: $query) {
      affectedCount
    }
  }
`,ie=`
  mutation removeFromTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    removeFromTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,ae=`
  mutation addToTags($type: DataType!, $tagIds: [ID!]!, $query: String!) {
    addToTags(type: $type, tagIds: $tagIds, query: $query)
  }
`,oe=`
  mutation updateTagRelations($type: DataType!, $keys: [String!]!, $addTagIds: [ID!]!, $removeTagIds: [ID!]!) {
    updateTagRelations(type: $type, keys: $keys, addTagIds: $addTagIds, removeTagIds: $removeTagIds)
  }
`,se=`
  mutation createTag($type: DataType!, $name: String!) {
    createTag(type: $type, name: $name) {
      ...TagFragment
    }
  }
  ${a}
`,ce=`
  mutation updateTag($id: ID!, $name: String!) {
    updateTag(id: $id, name: $name) {
      ...TagFragment
    }
  }
  ${a}
`,le=`
  mutation deleteTag($id: ID!) {
    deleteTag(id: $id)
  }
`,ue=`
  mutation addFavoriteFolder($rootPath: String!, $fullPath: String!) {
    addFavoriteFolder(rootPath: $rootPath, fullPath: $fullPath) {
      rootPath
      fullPath
    }
  }
`,de=`
  mutation removeFavoriteFolder($fullPath: String!) {
    removeFavoriteFolder(fullPath: $fullPath) {
      rootPath
      fullPath
      alias
    }
  }
`,fe=`
  mutation setMountAlias($id: ID!, $alias: String!) {
    setMountAlias(id: $id, alias: $alias)
  }
`,pe=`
  mutation setFavoriteFolderAlias($fullPath: String!, $alias: String!) {
    setFavoriteFolderAlias(fullPath: $fullPath, alias: $alias) {
      rootPath
      fullPath
      alias
    }
  }
`,me=`
  mutation createNote($input: NoteInput!) {
    createNote(input: $input) {
      ...NoteFragment
    }
  }
  ${m}
`,he=`
  mutation updateNote($id: ID!, $input: NoteInput!) {
    updateNote(id: $id, input: $input) {
      ...NoteFragment
    }
  }
  ${m}
`,ge=`
  mutation deleteNotes($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,_e=`
  mutation trashNotes($query: String!) {
    trashNotes(query: $query) {
      affectedCount
    }
  }
`,ve=`
  mutation restoreNotes($query: String!) {
    restoreNotes(query: $query) {
      affectedCount
    }
  }
`,ye=`
  mutation deleteFeedEntries($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,be=`
  mutation deleteCalls($query: String!) {
    deleteCalls(query: $query) {
      affectedCount
    }
  }
`,xe=`
  mutation deleteContacts($query: String!) {
    deleteContacts(query: $query) {
      affectedCount
    }
  }
`,Se=`
  mutation createFeed($url: String!, $fetchContent: Boolean!) {
    createFeed(url: $url, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${c}
`,Ce=`
  mutation importFeeds($content: String!) {
    importFeeds(content: $content)
  }
`,we=`
  mutation relaunchApp {
    relaunchApp
  }
`,Te=`
  mutation openAccessibilitySettings {
    openAccessibilitySettings
  }
`,Ee=`
  mutation requestAccess($feature: AccessFeature!) {
    requestAccess(feature: $feature)
  }
`,De=`
  mutation deleteFeed($id: ID!) {
    deleteFeed(id: $id)
  }
`,Oe=`
  mutation updateFeed($id: ID!, $name: String!, $fetchContent: Boolean!) {
    updateFeed(id: $id, name: $name, fetchContent: $fetchContent) {
      ...FeedFragment
    }
  }
  ${c}
`,ke=`
  mutation syncFeeds($id: ID) {
    syncFeeds(id: $id)
  }
`,Ae=`
  mutation syncFeedEntryContent($id: ID!) {
    syncFeedEntryContent(id: $id) {
      ...FeedEntryFragment
      feed {
        ...FeedFragment
      }
    }
  }
  ${c}
  ${s}
`,je=`
  mutation call($number: String!, $showDialer: Boolean!) {
    call(number: $number, showDialer: $showDialer)
  }
`,Me=`
  mutation setClipboard($text: String!) {
    setClipboard(text: $text)
  }
`,Ne=`
  mutation sendSms($number: String!, $body: String!, $attachmentPaths: [String!]! = [], $subscriptionId: Int, $requestId: String!) {
    sendSms(number: $number, body: $body, attachmentPaths: $attachmentPaths, subscriptionId: $subscriptionId, requestId: $requestId) {
      requestId
      requiresConfirmation
    }
  }
`,Pe=`
  mutation archiveSmsConversation($id: String!) {
    archiveSmsConversation(id: $id)
  }
`,Fe=`
  mutation unarchiveSmsConversation($id: String!) {
    unarchiveSmsConversation(id: $id)
  }
`,Ie=`
  mutation uninstallPackages($id: ID!) {
    uninstallPackages(ids: [$id])
  }
`,Le=`
  mutation installPackage($path: String!) {
    installPackage(path: $path) {
      id
      updatedAt
      isNew
    }
  }
`,Re=`
  mutation startScreenMirror($audio: Boolean!) {
    startScreenMirror(audio: $audio)
  }
`,ze=`
  mutation requestScreenMirrorAudio {
    requestScreenMirrorAudio
  }
`,Be=`
  mutation stopScreenMirror {
    stopScreenMirror
  }
`,Ve=`
  mutation setTempValue($key: String!, $value: String!) {
    setTempValue(key: $key, value: $value) {
      key
      value
    }
  }
`,He=`
  mutation deleteNotifications($ids: [ID!]!) {
    deleteNotifications(ids: $ids) {
      affectedCount
    }
  }
`,Ue=`
  mutation replyNotification($id: ID!, $actionIndex: Int!, $text: String!) {
    replyNotification(id: $id, actionIndex: $actionIndex, text: $text)
  }
`,We=`
  mutation deleteClipboardItems($query: String!) {
    deleteClipboardItems(query: $query) {
      affectedCount
    }
  }
`,Ge=`
  mutation updateScreenMirrorQuality($mode: ScreenMirrorMode!) {
    updateScreenMirrorQuality(mode: $mode)
  }
`,Ke=`
  mutation saveFeedEntriesToNotes($query: String!) {
    saveFeedEntriesToNotes(query: $query)
  }
`,qe=`
  mutation mergeChunks($fileId: String!, $totalChunks: Int!, $path: String!, $replace: Boolean!, $totalSize: Long!) {
    mergeChunks(fileId: $fileId, totalChunks: $totalChunks, path: $path, replace: $replace, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Je=`
  mutation mergeAppFileChunks($fileId: String!, $totalChunks: Int!, $fileName: String!, $totalSize: Long!) {
    mergeAppFileChunks(fileId: $fileId, totalChunks: $totalChunks, fileName: $fileName, totalSize: $totalSize) {
      status
      value
      mergedSize
      error
    }
  }
`,Ye=`
  mutation deleteChunks($fileId: String!) {
    deleteChunks(fileId: $fileId)
  }
`,Xe=`
  mutation startPomodoro($durationSec: Int!) {
    startPomodoro(durationSec: $durationSec) { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Ze=`
  mutation stopPomodoro {
    stopPomodoro { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,Qe=`
  mutation pausePomodoro {
    pausePomodoro { date completedCount currentRound timeLeftSec totalTimeSec isRunning isPaused state }
  }
`,$e=`mutation logout { logout }`,et=`
  mutation importBookmarks($content: String!) {
    importBookmarks(content: $content) { affectedCount }
  }
`,tt=`
  mutation addBookmarks($urls: [String!]!, $groupId: ID!) {
    addBookmarks(urls: $urls, groupId: $groupId) {
      ...BookmarkFragment
    }
  }
  ${p}
`,nt=`
  mutation updateBookmark($id: ID!, $input: BookmarkInput!) {
    updateBookmark(id: $id, input: $input) {
      ...BookmarkFragment
    }
  }
  ${p}
`,rt=`
  mutation deleteBookmarks($ids: [ID!]!) {
    deleteBookmarks(ids: $ids) {
      affectedCount
    }
  }
`,it=`
  mutation recordBookmarkClick($id: ID!) {
    recordBookmarkClick(id: $id)
  }
`,at=`
  mutation createBookmarkGroup($name: String!) {
    createBookmarkGroup(name: $name) {
      ...BookmarkGroupFragment
    }
  }
  ${l}
`,ot=`
  mutation updateBookmarkGroup($id: ID!, $name: String!, $collapsed: Boolean!, $sortOrder: Int!) {
    updateBookmarkGroup(id: $id, name: $name, collapsed: $collapsed, sortOrder: $sortOrder) {
      ...BookmarkGroupFragment
    }
  }
  ${l}
`,st=`
  mutation deleteBookmarkGroup($id: ID!) {
    deleteBookmarkGroup(id: $id)
  }
`,ct=`
  mutation deleteFiles($paths: [String!]!) {
    deleteFiles(paths: $paths) {
      affectedCount
    }
  }
`,lt=`
  mutation { importImageSearchModel }
`,ut=`
  mutation { enableImageSearch }
`,dt=`
  mutation { disableImageSearch }
`,ft=`
  mutation { cancelImageModelDownload }
`,pt=`
  mutation startImageIndex($force: Boolean) {
    startImageIndex(force: $force)
  }
`,mt=`
  mutation { cancelImageIndex }
`,ht=`
  mutation createContact($input: ContactInput!) {
    createContact(input: $input) {
      ...ContactFragment
    }
  }
  ${o}
`,gt=`
  mutation updateContact($id: ID!, $input: ContactInput!) {
    updateContact(id: $id, input: $input) {
      ...ContactFragment
    }
  }
  ${o}
`,_t=`
  mutation DeleteNote($query: String!) {
    deleteNotes(query: $query) {
      affectedCount
    }
  }
`,vt=`
  mutation deleteFeedEntry($query: String!) {
    deleteFeedEntries(query: $query) {
      affectedCount
    }
  }
`,yt=`
  mutation removePref($key: String!) {
    removePref(key: $key)
  }
`,$=`
  mutation DeleteDbTableRows($table: String!, $ids: [String!]!) {
    deleteDbTableRows(table: $table, ids: $ids)
  }
`,bt=`
  mutation StartDiscovery {
    startDiscovery
  }
`,xt=`
  mutation StopDiscovery {
    stopDiscovery
  }
`,St=`
  mutation PairDevice($peerId: ID!, $input: PairPeerInput!) {
    pairPeer(peerId: $peerId, input: $input)
  }
`,Ct=`
  mutation cancelPairing($peerId: ID!) {
    cancelPairing(peerId: $peerId)
  }
`,wt=`
  mutation respondToPairing($peerId: ID!, $accepted: Boolean!) {
    respondToPairing(peerId: $peerId, accepted: $accepted)
  }
`,Tt=`
  mutation createDownload($input: DownloadInput!) {
    createDownload(input: $input) { id }
  }
`,Et=`
  mutation pauseDownload($id: ID!) { pauseDownload(id: $id) }
`,Dt=`
  mutation resumeDownload($id: ID!) { resumeDownload(id: $id) }
`,Ot=`
  mutation retryDownload($id: ID!) { retryDownload(id: $id) }
`,kt=`
  mutation pauseFileIndex($volumeId: ID) { pauseFileIndex(volumeId: $volumeId) }
`,At=`
  mutation resumeFileIndex($volumeId: ID) { resumeFileIndex(volumeId: $volumeId) }
`,jt=`
  mutation stopFileIndex($volumeId: ID) { stopFileIndex(volumeId: $volumeId) }
`,Mt=`
  mutation formatDisk($path: String!) {
    formatDisk(path: $path)
  }
`;function Nt(e){t.emit(`toast`,e instanceof n?e.message:r.NetworkError)}export{Ce as $,Ae as $t,x as A,Dt as At,J as B,Me as Bt,me as C,Ue as Ct,rt as D,X as Dt,st as E,wt as Et,$ as F,Ke as Ft,re as G,bt as Gt,ge as H,fe as Ht,ye as I,B as It,ut as J,Re as Jt,le as K,pt as Kt,vt as L,V as Lt,Ye as M,C as Mt,We as N,Ot as Nt,be as O,ve as Ot,xe as P,h as Pt,et as Q,Be as Qt,De as R,b as Rt,R as S,q as St,N as T,ze as Tt,He as U,_ as Ut,_t as V,pe as Vt,D as W,Ve as Wt,Mt as X,jt as Xt,H as Y,xt as Yt,y as Z,Ze as Zt,w as _,oe as _n,j as _t,A as a,Ie as an,Je as at,Tt as b,yt as bt,Pe as c,nt as cn,Te as ct,ft as d,gt as dn,kt as dt,ke as en,lt as et,Ct as f,v as fn,Qe as ft,at as g,ce as gn,W as gt,L as h,Ge as hn,we as ht,tt as i,Fe as in,$e as it,S as j,At as jt,E as k,ne as kt,je as l,ot as ln,St as lt,K as m,he as mn,z as mt,M as n,_e as nn,Le as nt,ue as o,O as on,qe as ot,g as p,Oe as pn,it as pt,dt as q,Xe as qt,G as r,Q as rn,k as rt,ae as s,U as sn,Z as st,Nt as t,Y as tn,te as tt,mt as u,T as un,Et as ut,ht as v,F as vn,de as vt,se as w,Ee as wt,Se as x,I as xt,P as y,ie as yt,ct as z,Ne as zt};