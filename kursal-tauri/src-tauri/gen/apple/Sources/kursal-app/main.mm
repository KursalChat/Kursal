#import <UIKit/UIKit.h>
#import <WebKit/WebKit.h>
#include "bindings/bindings.h"

static WKWebView *findWebView(UIView *view) {
	if ([view isKindOfClass:[WKWebView class]]) return (WKWebView *)view;
	for (UIView *sub in view.subviews) {
		WKWebView *found = findWebView(sub);
		if (found) return found;
	}
	return nil;
}

static UIWindow *appWindow(void) {
	UIWindow *first = nil;
	for (UIScene *scene in UIApplication.sharedApplication.connectedScenes) {
		if (![scene isKindOfClass:[UIWindowScene class]]) continue;
		for (UIWindow *w in ((UIWindowScene *)scene).windows) {
			if (!first) first = w;
			if (w.isKeyWindow) return w;
		}
	}
	return first;
}

@interface KBOffsetPin : NSObject
@end
@implementation KBOffsetPin
- (void)observeValueForKeyPath:(NSString *)keyPath
                      ofObject:(id)object
                        change:(NSDictionary *)change
                       context:(void *)context {
	UIScrollView *sv = (UIScrollView *)object;
	if (!CGPointEqualToPoint(sv.contentOffset, CGPointZero)) {
		sv.contentOffset = CGPointZero;
	}
}
@end

static KBOffsetPin *pin = nil;
static WKWebView *pinnedWebView = nil;

static void handleKeyboard(NSNotification *note) {
	UIWindow *window = appWindow();
	if (!window) return;
	WKWebView *webView = findWebView(window);
	if (!webView || !webView.superview) return;

	if (pinnedWebView != webView) {
		[pinnedWebView.scrollView removeObserver:pin forKeyPath:@"contentOffset"];
		[webView.scrollView addObserver:pin forKeyPath:@"contentOffset" options:0 context:NULL];
		pinnedWebView = webView;
	}

	CGRect end = [note.userInfo[UIKeyboardFrameEndUserInfoKey] CGRectValue];
	CGRect endLocal = [webView.superview convertRect:end
	                             fromCoordinateSpace:window.screen.coordinateSpace];
	CGRect bounds = webView.superview.bounds;

	CGFloat overlap = 0;
	if (CGRectGetMaxY(endLocal) >= CGRectGetMaxY(bounds) - 1) {
		overlap = MIN(CGRectGetHeight(endLocal),
		              MAX(0, CGRectGetMaxY(bounds) - CGRectGetMinY(endLocal)));
	}

	CGRect target = bounds;
	target.size.height -= overlap;
	if (CGRectEqualToRect(webView.frame, target)) return;

	if (@available(iOS 15.0, *)) {
		webView.superview.backgroundColor = webView.underPageBackgroundColor;
	}

	double duration = [note.userInfo[UIKeyboardAnimationDurationUserInfoKey] doubleValue];
	UIViewAnimationOptions curve = (UIViewAnimationOptions)
	    ([note.userInfo[UIKeyboardAnimationCurveUserInfoKey] unsignedIntegerValue] << 16);
	[UIView animateWithDuration:duration
	                      delay:0
	                    options:(curve | UIViewAnimationOptionBeginFromCurrentState)
	                 animations:^{ webView.frame = target; }
	                 completion:nil];
}

static void installKeyboardFrameHandler(void) {
	pin = [KBOffsetPin new];
	[[NSNotificationCenter defaultCenter]
	    addObserverForName:UIKeyboardWillChangeFrameNotification
	                object:nil
	                 queue:nil
	            usingBlock:^(NSNotification *note) { handleKeyboard(note); }];
}

int main(int argc, char * argv[]) {
	installKeyboardFrameHandler();
	ffi::start_app();
	return 0;
}
