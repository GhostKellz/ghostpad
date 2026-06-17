// KWin blur helper for GhostPad
// Enables background blur effects on KDE Plasma

#ifndef KWIN_BLUR_H
#define KWIN_BLUR_H

#include <QObject>

class QWindow;

class KWinBlurHelper : public QObject
{
    Q_OBJECT

public:
    explicit KWinBlurHelper(QObject *parent = nullptr);

    Q_INVOKABLE void enableBlur(QWindow *window, bool enable);
    Q_INVOKABLE bool isAvailable() const;

private:
    bool m_kwinAvailable;
};

#endif // KWIN_BLUR_H
