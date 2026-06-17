// KWin blur helper implementation

#include "kwin_blur.h"
#include <QWindow>

#ifdef HAS_KWINDOWEFFECTS
#include <KWindowEffects>
#endif

KWinBlurHelper::KWinBlurHelper(QObject *parent)
    : QObject(parent)
    , m_kwinAvailable(false)
{
#ifdef HAS_KWINDOWEFFECTS
    m_kwinAvailable = true;
#endif
}

void KWinBlurHelper::enableBlur(QWindow *window, bool enable)
{
    if (!window) {
        return;
    }

#ifdef HAS_KWINDOWEFFECTS
    KWindowEffects::enableBlurBehind(window, enable);
#else
    Q_UNUSED(enable);
#endif
}

bool KWinBlurHelper::isAvailable() const
{
    return m_kwinAvailable;
}
