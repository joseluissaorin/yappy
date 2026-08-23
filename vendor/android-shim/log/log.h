/* Shim para fdk-aac en el NDK: upstream incluye el header INTERNO de AOSP
   "log/log.h" (no existe en el NDK). Solo usa android_errorWriteLog para
   avisar de streams AAC raros; lo mapeamos al log público del NDK. */
#pragma once
#include <android/log.h>
#define android_errorWriteLog(tag, subTag) \
    __android_log_print(ANDROID_LOG_ERROR, "fdk-aac", "%s", (subTag))
