//
//	Copyright (C) 2013 Hong Jen Yee (PCMan) <pcman.tw@gmail.com>
//
//	This library is free software; you can redistribute it and/or
//	modify it under the terms of the GNU Library General Public
//	License as published by the Free Software Foundation; either
//	version 2 of the License, or (at your option) any later version.
//
//	This library is distributed in the hope that it will be useful,
//	but WITHOUT ANY WARRANTY; without even the implied warranty of
//	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
//	Library General Public License for more details.
//
//	You should have received a copy of the GNU Library General Public
//	License along with this library; if not, write to the
//	Free Software Foundation, Inc., 51 Franklin St, Fifth Floor,
//	Boston, MA  02110-1301, USA.
//

#include "TextService.h"
#include "EditSession.h"
#include "LangBarButton.h"
#include "libime2.h"

#include <assert.h>
#include <msctf.h>
#include <winerror.h>
#include <winrt/base.h>
#include <string>

using namespace std;

extern HINSTANCE g_hInstance;

// eea32958-dc57-4542-9fc8-33c74f5caaa9
static const GUID g_inputDisplayAttributeGuid = {
    0xeea32958,
    0xdc57,
    0x4542,
    {0x9f, 0xc8, 0x33, 0xc7, 0x4f, 0x5c, 0xaa, 0xa9}
};

namespace Ime {

TextService::TextService():
	threadMgr_(NULL),
	clientId_(TF_CLIENTID_NULL),
	activateFlags_(0),
	isKeyboardOpened_(false),
	threadMgrEventSinkCookie_(TF_INVALID_COOKIE),
	textEditSinkCookie_(TF_INVALID_COOKIE),
	compositionSinkCookie_(TF_INVALID_COOKIE),
	keyboardOpenEventSinkCookie_(TF_INVALID_COOKIE),
	langBarSinkCookie_(TF_INVALID_COOKIE),
	activateLanguageProfileNotifySinkCookie_(TF_INVALID_COOKIE),
	composition_(NULL),
	input_atom_(TF_INVALID_GUIDATOM),
	refCount_(1) {
	addCompartmentMonitor(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);

	// FIXME we should only initialize once
	LibIME2Init();
	ImeWindowRegisterClass(g_hInstance);

	TF_DISPLAYATTRIBUTE da = {
		{TF_CT_NONE, {}},  // text color
		{TF_CT_NONE, {}},  // background color
		TF_LS_DOT,         // underline style
		FALSE,             // underline boldness
		{TF_CT_NONE, {}},  // underline color
		TF_ATTR_INPUT      // attribute info
	};
	RegisterDisplayAttribute(&g_inputDisplayAttributeGuid, da, &input_atom_);
}

TextService::~TextService(void) {

	// This should only happen in rare cases
	if(!compartmentMonitors_.empty()) {
		vector<CompartmentMonitor>::iterator it;
		for(it = compartmentMonitors_.begin(); it != compartmentMonitors_.end(); ++it) {
			winrt::com_ptr<ITfSource> source = threadCompartment(it->guid).as<ITfSource>();
			if (source) {
				source->UnadviseSink(it->cookie);
			}
		}
	}

	if(langBarMgr_) {
		langBarMgr_->UnadviseEventSink(langBarSinkCookie_);
	}
	langBarMgr_ = NULL;
}

// public methods

ITfThreadMgr* TextService::threadMgr() const {
	return threadMgr_.get();
}

TfClientId TextService::clientId() const {
	return clientId_;
}


// language bar
DWORD TextService::langBarStatus() const {
	if(langBarMgr_) {
		DWORD status;
		if(langBarMgr_->GetShowFloatingStatus(&status) == S_OK) {
			return status;
		}
	}
	return 0;
}

void TextService::addButton(LangBarButton* button) {
	if(button) {
		winrt::com_ptr<LangBarButton> btn;
		btn.copy_from(button);
		langBarButtons_.emplace_back(btn);
		if(isActivated()) {
			winrt::com_ptr<ITfLangBarItemMgr> langBarItemMgr;
			if(threadMgr_->QueryInterface(IID_ITfLangBarItemMgr, langBarItemMgr.put_void()) == S_OK) {
				langBarItemMgr->AddItem(button);
			}
		}
	}
}

void TextService::removeButton(LangBarButton* button) {
	if(button) {
		winrt::com_ptr<LangBarButton> btn;
		btn.copy_from(button);
		auto it = find(langBarButtons_.begin(), langBarButtons_.end(), btn);
		if(it != langBarButtons_.end()) {
			if(isActivated()) {
				winrt::com_ptr<ITfLangBarItemMgr> langBarItemMgr;
				if(threadMgr_->QueryInterface(IID_ITfLangBarItemMgr, langBarItemMgr.put_void()) == S_OK) {
					langBarItemMgr->RemoveItem(button);
				}
			}
			langBarButtons_.erase(it);
		}
	}
}

// preserved key
void TextService::addPreservedKey(UINT keyCode, UINT modifiers, const GUID& guid) {
	PreservedKey preservedKey;
	preservedKey.guid = guid;
	preservedKey.uVKey = keyCode;
	preservedKey.uModifiers = modifiers;
	preservedKeys_.push_back(preservedKey);
	if(threadMgr_) { // our text service is activated
		ITfKeystrokeMgr *keystrokeMgr;
		if (threadMgr_->QueryInterface(IID_ITfKeystrokeMgr, (void **)&keystrokeMgr) == S_OK) {
			keystrokeMgr->PreserveKey(clientId_, guid, &preservedKey, NULL, 0);
			keystrokeMgr->Release();
		}
	}
}

void TextService::removePreservedKey(const GUID& guid) {
	vector<PreservedKey>::iterator it;
	for(it = preservedKeys_.begin(); it != preservedKeys_.end(); ++it) {
		PreservedKey& preservedKey = *it;
		if(::IsEqualIID(preservedKey.guid, guid)) {
			if(threadMgr_) { // our text service is activated
				ITfKeystrokeMgr *keystrokeMgr;
				if (threadMgr_->QueryInterface(IID_ITfKeystrokeMgr, (void **)&keystrokeMgr) == S_OK) {
					keystrokeMgr->UnpreserveKey(preservedKey.guid, &preservedKey);
					keystrokeMgr->Release();
				}
			}
			preservedKeys_.erase(it);
			break;
		}
	}
}


// text composition

bool TextService::isComposing() {
	return (composition_ != NULL);
}

// is keyboard disabled for the context (NULL means current context)
bool TextService::isKeyboardDisabled(ITfContext* context) {
	return (contextCompartmentValue(GUID_COMPARTMENT_KEYBOARD_DISABLED, context)
		|| contextCompartmentValue(GUID_COMPARTMENT_EMPTYCONTEXT, context));
}

// is keyboard opened for the whole thread
bool TextService::isKeyboardOpened() {
	return isKeyboardOpened_;
}

void TextService::setKeyboardOpen(bool open) {
	if(open != isKeyboardOpened_) {
		setThreadCompartmentValue(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE, (DWORD)open);
	}
}


// check if current insertion point is in the range of composition.
// if not in range, insertion is now allowed
bool TextService::isInsertionAllowed(EditSession* session) {
	TfEditCookie cookie = session->editCookie();
	TF_SELECTION selection;
	ULONG selectionNum;
	if(isComposing()) {
		if(session->context()->GetSelection(cookie, TF_DEFAULT_SELECTION, 1, &selection, &selectionNum) == S_OK) {
			ITfRange* compositionRange;
			if(composition_->GetRange(&compositionRange) == S_OK) {
				bool allowed = false;
				// check if current selection is covered by composition range
				LONG compareResult1;
				LONG compareResult2;
				if(selection.range->CompareStart(cookie, compositionRange, TF_ANCHOR_START, &compareResult1) == S_OK
					&& selection.range->CompareStart(cookie, compositionRange, TF_ANCHOR_END, &compareResult2) == S_OK) {
					if(compareResult1 == -1 && compareResult2 == +1)
						allowed = true;
				}
				compositionRange->Release();
			}
			if(selection.range)
				selection.range->Release();
		}
	}
	return false;
}

void TextService::startComposition(ITfContext* context) {
	assert(context);
	HRESULT sessionResult;
	StartCompositionEditSession* session = new StartCompositionEditSession(this, context);
	context->RequestEditSession(clientId_, session, TF_ES_SYNC|TF_ES_READWRITE, &sessionResult);
	session->Release();
}

void TextService::endComposition(ITfContext* context) {
	assert(context);
	HRESULT sessionResult;
	EndCompositionEditSession* session = new EndCompositionEditSession(this, context);
	context->RequestEditSession(clientId_, session, TF_ES_SYNC|TF_ES_READWRITE, &sessionResult);
	session->Release();
}

std::wstring TextService::compositionString(EditSession* session) {
	if (composition_) {
		winrt::com_ptr<ITfRange> compositionRange;
		if (composition_->GetRange(compositionRange.put()) == S_OK) {
			TfEditCookie editCookie = session->editCookie();
			// FIXME: the TSF API is really stupid here and it provides no way to know the size of the range.
			// we cannot even get the actual position of start and end to calculate by ourselves.
			// So, just use a huge buffer and assume that it's enough. :-(
			// this should be quite enough for most of the IME on the earth as most composition strings
			// only contain dozens of characters.
			wchar_t buf[4096];  
			ULONG len = 0;
			if (compositionRange->GetText(editCookie, 0, buf, 4096, &len) == S_OK) {
				buf[len] = '\0';
				return std::wstring(buf);
			}
		}
	}
	return std::wstring();
}

void TextService::setCompositionString(EditSession* session, const wchar_t* str, int len) {
	ITfContext* context = session->context();
	if (context) {
		TfEditCookie editCookie = session->editCookie();
		winrt::com_ptr<ITfRange> compositionRange;
		if(composition_->GetRange(compositionRange.put()) == S_OK) {
			// replace context of composion area with the new string.
			compositionRange->SetText(editCookie, 0, str, len);

			// set display attribute to the composition range
			winrt::com_ptr<ITfProperty> dispAttrProp;
			if(context->GetProperty(GUID_PROP_ATTRIBUTE, dispAttrProp.put()) == S_OK) {
				VARIANT var;
				VariantInit(&var);
				var.vt = VT_I4;
				var.lVal = input_atom_;
				dispAttrProp->SetValue(editCookie, compositionRange.get(), &var);
			}
		}
	}
}

// set cursor position in the composition area
// 0 means the start pos of composition string
void TextService::setCompositionCursor(EditSession* session, int pos) {
	TF_SELECTION selection;
	ULONG selectionNum;
	// get current selection
	if(session->context()->GetSelection(session->editCookie(), TF_DEFAULT_SELECTION, 1, &selection, &selectionNum) == S_OK) {
		// get composition range
		ITfRange* compositionRange;
		if(composition_->GetRange(&compositionRange) == S_OK) {
			// make the start of selectionRange the same as that of compositionRange
			selection.range->ShiftStartToRange(session->editCookie(), compositionRange, TF_ANCHOR_START);
			selection.range->Collapse(session->editCookie(), TF_ANCHOR_START);
			LONG moved;
			// move the start anchor to right
			selection.range->ShiftStart(session->editCookie(), (LONG)pos, &moved, NULL);
			selection.range->Collapse(session->editCookie(), TF_ANCHOR_START);
			// set the new selection to the context
			session->context()->SetSelection(session->editCookie(), 1, &selection);
			compositionRange->Release();
		}
		selection.range->Release();
	}
}

// compartment handling
winrt::com_ptr<ITfCompartment> TextService::threadCompartment(const GUID& key) {
	if(threadMgr_) {
		winrt::com_ptr<ITfCompartmentMgr> compartmentMgr = threadMgr_.as<ITfCompartmentMgr>();
		if(compartmentMgr) {
			winrt::com_ptr<ITfCompartment> compartment;
			compartmentMgr->GetCompartment(key, compartment.put());
			return compartment;
		}
	}
	return NULL;
}

winrt::com_ptr<ITfCompartment> TextService::contextCompartment(const GUID& key, ITfContext* context) {
	winrt::com_ptr<ITfContext> curContext;
	if(!context) {
		curContext.attach(currentContext());
	} else {
		curContext.copy_from(context);
	}
	if(context) {
		winrt::com_ptr<ITfCompartmentMgr> compartmentMgr = curContext.as<ITfCompartmentMgr>();
		if(compartmentMgr) {
			winrt::com_ptr<ITfCompartment> compartment;
			compartmentMgr->GetCompartment(key, compartment.put());
			return compartment;
		}
	}
	return NULL;
}

DWORD TextService::threadCompartmentValue(const GUID& key) {
	winrt::com_ptr<ITfCompartment> compartment = threadCompartment(key);
	if(compartment) {
		VARIANT var;
		::VariantInit(&var);
		HRESULT r = compartment->GetValue(&var);
		if(r == S_OK) {
			if(var.vt == VT_I4)
				return (DWORD)var.lVal;
		}
	}
	return 0;
}

DWORD TextService::contextCompartmentValue(const GUID& key, ITfContext* context) {
	winrt::com_ptr<ITfCompartment> compartment = contextCompartment(key, context);
	if(compartment) {
		VARIANT var;
		if(compartment->GetValue(&var) == S_OK && var.vt == VT_I4) {
			return (DWORD)var.lVal;
		}
	}
	return 0;
}

void TextService::setThreadCompartmentValue(const GUID& key, DWORD value) {
	winrt::com_ptr<ITfCompartment> compartment = threadCompartment(key);
	if(compartment) {
		VARIANT var;
		::VariantInit(&var);
		var.vt = VT_I4;
		var.lVal = value;
		compartment->SetValue(clientId_, &var);
	}
}

void TextService::addCompartmentMonitor(const GUID key) {
	CompartmentMonitor monitor;
	monitor.guid = key;
	monitor.cookie = 0;
	// if the text service is activated
	if(threadMgr_) {
		winrt::com_ptr<ITfSource> source = threadCompartment(key).as<ITfSource>();
		if(source) {
			source->AdviseSink(IID_ITfCompartmentEventSink, (ITfCompartmentEventSink*)this, &monitor.cookie);
		}
	}
	compartmentMonitors_.push_back(monitor);
}

void TextService::removeCompartmentMonitor(const GUID key) {
	vector<CompartmentMonitor>::iterator it;
	it = find(compartmentMonitors_.begin(), compartmentMonitors_.end(), key);
	if(it != compartmentMonitors_.end()) {
		if(threadMgr_) {
			winrt::com_ptr<ITfSource> source = threadCompartment(key).as<ITfSource>();
			source->UnadviseSink(it->cookie);
		}
		compartmentMonitors_.erase(it);
	}
}

// virtual
void TextService::onCompartmentChanged(const GUID& key) {
	// keyboard status changed, this is threadMgr specific
	// See explanations on TSF aware blog:
	// http://blogs.msdn.com/b/tsfaware/archive/2007/05/30/what-is-a-keyboard.aspx
	if(::IsEqualGUID(key, GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)) {
		isKeyboardOpened_ = threadCompartmentValue(key) ? true : false;
		onKeyboardStatusChanged(isKeyboardOpened_);
	}
}

// COM stuff

// IUnknown
STDMETHODIMP TextService::QueryInterface(REFIID riid, void **ppvObj) {
	// XXX MS document says "The TSF manager obtains an instance of this
	// interface by calling CoCreateInstance with the class identifier
	// passed to ITfCategoryMgr::RegisterCategory with GUID_TFCAT_DISPLAYATTRIBUTEPROVIDER
	// and IID_ITfDisplayAttributeProvider. For more information, see
	// Providing Display Attributes." However, in practice the DisplayAttributeMgr
	// directly queries the text service object for the interface, so we need
	// to handle the query interface here.
	if (IsEqualIID(riid, IID_ITfDisplayAttributeProvider)) {
        CreateDisplayAttributeProvider(ppvObj);
		return S_OK;
	}

    if (ppvObj == NULL)
        return E_INVALIDARG;
	if(IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_ITfTextInputProcessor))
		*ppvObj = (ITfTextInputProcessor*)this;
	else if(IsEqualIID(riid, IID_ITfTextInputProcessorEx))
		*ppvObj = (ITfTextInputProcessorEx*)this;
	else if(IsEqualIID(riid, IID_ITfThreadMgrEventSink))
		*ppvObj = (ITfThreadMgrEventSink*)this;
	else if(IsEqualIID(riid, IID_ITfTextEditSink))
		*ppvObj = (ITfTextEditSink*)this;
	else if(IsEqualIID(riid, IID_ITfKeyEventSink))
		*ppvObj = (ITfKeyEventSink*)this;
	else if(IsEqualIID(riid, IID_ITfCompositionSink))
		*ppvObj = (ITfCompositionSink*)this;
	else if(IsEqualIID(riid, IID_ITfCompartmentEventSink))
		*ppvObj = (ITfCompartmentEventSink*)this;
	else
		*ppvObj = NULL;

	if(*ppvObj) {
		AddRef();
		return S_OK;
	}
	return E_NOINTERFACE;
}

// IUnknown implementation
STDMETHODIMP_(ULONG) TextService::AddRef(void) {
	return ++refCount_;
}

STDMETHODIMP_(ULONG) TextService::Release(void) {
	assert(refCount_ > 0);
	const ULONG newCount = --refCount_;
	if(0 == refCount_) {
		delete this;
	}
	return newCount;
}

// ITfTextInputProcessor
STDMETHODIMP TextService::Activate(ITfThreadMgr *pThreadMgr, TfClientId tfClientId) {
	// store tsf manager & client id
	threadMgr_.copy_from(pThreadMgr);
	clientId_ = tfClientId;

	activateFlags_ = 0;
	winrt::com_ptr<ITfThreadMgrEx> threadMgrEx = threadMgr_.as<ITfThreadMgrEx>();
	if(threadMgrEx) {
		threadMgrEx->GetActiveFlags(&activateFlags_);
	}

	// advice event sinks (set up event listeners)
	
	// ITfThreadMgrEventSink
	winrt::com_ptr<ITfSource> source = threadMgr_.as<ITfSource>();
	if(source) {
		source->AdviseSink(IID_ITfThreadMgrEventSink, (ITfThreadMgrEventSink *)this, &threadMgrEventSinkCookie_);
	}

	// ITfTextEditSink,

	// ITfKeyEventSink
	winrt::com_ptr<ITfKeystrokeMgr> keystrokeMgr = threadMgr_.as<ITfKeystrokeMgr>();
	if(keystrokeMgr)
		keystrokeMgr->AdviseKeyEventSink(clientId_, (ITfKeyEventSink*)this, TRUE);

	// register preserved keys
	if(!preservedKeys_.empty()) {
		vector<PreservedKey>::iterator it;
		for(it = preservedKeys_.begin(); it != preservedKeys_.end(); ++it) {
			PreservedKey& preservedKey = *it;
			keystrokeMgr->PreserveKey(clientId_, preservedKey.guid, &preservedKey, NULL, 0);
		}
	}

	// ITfCompositionSink

	// ITfCompartmentEventSink
	// get current keyboard state
	if(!compartmentMonitors_.empty()) {
		vector<CompartmentMonitor>::iterator it;
		for(it = compartmentMonitors_.begin(); it != compartmentMonitors_.end(); ++it) {
			winrt::com_ptr<ITfSource> source = threadCompartment(it->guid).as<ITfSource>();
			source->AdviseSink(IID_ITfCompartmentEventSink, (ITfCompartmentEventSink*)this, &it->cookie);
		}
	}
	isKeyboardOpened_ = threadCompartmentValue(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) != 0;

	// FIXME: under Windows 7, it seems that the keyboard is closed every time
	// our text service is activated. The value in the compartment is always empty. :-(
	// So, we open the keyboard manually here, but I'm not sure if this is the correct behavior.
	if(!isKeyboardOpened_)
		setKeyboardOpen(true);

	// initialize language bar
	::CoCreateInstance(CLSID_TF_LangBarMgr, NULL, CLSCTX_INPROC_SERVER,
                      IID_ITfLangBarMgr, (void**)&langBarMgr_);
	// Note: language bar has no effects in Win 8 immersive mode
	if(!langBarButtons_.empty()) {
		winrt::com_ptr<ITfLangBarItemMgr> langBarItemMgr;
		if(threadMgr_->QueryInterface(IID_ITfLangBarItemMgr, langBarItemMgr.put_void()) == S_OK) {
			for(auto& button: langBarButtons_) {
				langBarItemMgr->AddItem(button.get());
			}
		}
	}

	onActivate();
	//::MessageBox(0, L"onActivate", 0, 0);
	return S_OK;
}

STDMETHODIMP TextService::Deactivate() {
	//::MessageBox(0, L"Deactivate", 0, 0);
	// terminate composition properly
	if(isComposing()) {
		ITfContext* context = currentContext();
		if(context) {
			endComposition(context);
			context->Release();
		}
	}

	onDeactivate();

	// uninitialize language bar
	if(!langBarButtons_.empty()) {
		winrt::com_ptr<ITfLangBarItemMgr> langBarItemMgr;
		if(threadMgr_->QueryInterface(IID_ITfLangBarItemMgr, langBarItemMgr.put_void()) == S_OK) {
			for(auto& button: langBarButtons_) {
				langBarItemMgr->RemoveItem(button.get());
			}
		}
	}
	if(langBarMgr_) {
		langBarMgr_->UnadviseEventSink(langBarSinkCookie_);
		langBarSinkCookie_ = TF_INVALID_COOKIE;
		langBarMgr_ = NULL;
	}

	// unadvice event sinks

	// ITfThreadMgrEventSink
	winrt::com_ptr<ITfSource> source = threadMgr_.as<ITfSource>();
	if(source) {
		source->UnadviseSink(threadMgrEventSinkCookie_);
		source->UnadviseSink(activateLanguageProfileNotifySinkCookie_);
		threadMgrEventSinkCookie_ = TF_INVALID_COOKIE;
		activateLanguageProfileNotifySinkCookie_ = TF_INVALID_COOKIE;
	}

	// ITfTextEditSink,

	// ITfKeyEventSink
	winrt::com_ptr<ITfKeystrokeMgr> keystrokeMgr = threadMgr_.as<ITfKeystrokeMgr>();
	if(keystrokeMgr) {
		keystrokeMgr->UnadviseKeyEventSink(clientId_);
		// unregister preserved keys
		if(!preservedKeys_.empty()) {
			vector<PreservedKey>::iterator it;
			for(it = preservedKeys_.begin(); it != preservedKeys_.end(); ++it) {
				PreservedKey& preservedKey = *it;
				keystrokeMgr->UnpreserveKey(preservedKey.guid, &preservedKey);
			}
		}
	}

	// ITfCompositionSink

	// ITfCompartmentEventSink
	// thread specific compartment
	winrt::com_ptr<ITfCompartment> compartment = threadCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
	if(compartment) {
		winrt::com_ptr<ITfSource> compartmentSource = compartment.as<ITfSource>();
		if(compartmentSource)
			compartmentSource->UnadviseSink(keyboardOpenEventSinkCookie_);
		keyboardOpenEventSinkCookie_ = TF_INVALID_COOKIE;
	}

	threadMgr_ = NULL;
	clientId_ = TF_CLIENTID_NULL;
	activateFlags_ = 0;
	return S_OK;
}

// ITfTextInputProcessorEx
STDMETHODIMP TextService::ActivateEx(ITfThreadMgr *ptim, TfClientId tid, DWORD dwFlags) {
	Activate(ptim, tid);
	return S_OK;
}

// ITfThreadMgrEventSink
STDMETHODIMP TextService::OnInitDocumentMgr(ITfDocumentMgr *pDocMgr) {
	return S_OK;
}

STDMETHODIMP TextService::OnUninitDocumentMgr(ITfDocumentMgr *pDocMgr) {
	return S_OK;
}

STDMETHODIMP TextService::OnSetFocus(ITfDocumentMgr *pDocMgrFocus, ITfDocumentMgr *pDocMgrPrevFocus) {
	if(pDocMgrFocus != nullptr) {
		onSetFocus();
	} else {
		onKillFocus();
	}
	return S_OK;
}

STDMETHODIMP TextService::OnPushContext(ITfContext *pContext) {
	return S_OK;
}

STDMETHODIMP TextService::OnPopContext(ITfContext *pContext) {
	return S_OK;
}


// ITfTextEditSink
STDMETHODIMP TextService::OnEndEdit(ITfContext *pContext, TfEditCookie ecReadOnly, ITfEditRecord *pEditRecord) {
	// This method is called by the TSF whenever an edit operation ends.
	// It's possible for a document to have multiple composition strings at the
	// same time and it's possible for other text services to edit the same
	// document. Though such a complicated senario rarely exist, it indeed happen.

	// NOTE: I don't really know why this is needed and tests yielded no obvious effect
	// of this piece of code, but from MS TSF samples, this is needed.
	BOOL selChanged;
	if(pEditRecord->GetSelectionStatus(&selChanged) == S_OK) {
		if(selChanged && isComposing()) {
			// we need to check if current selection is in our composition string.
			// if after others' editing the selection (insertion point) has been changed and
			// fell outside our composition area, terminate the composition.
			TF_SELECTION selection;
			ULONG selectionNum;
			if(pContext->GetSelection(ecReadOnly, TF_DEFAULT_SELECTION, 1, &selection, &selectionNum) == S_OK) {
				winrt::com_ptr<ITfRange> compRange;
				if(composition_->GetRange(compRange.put()) == S_OK) {
					// check if two ranges overlaps
					// check if current selection is covered by composition range
					LONG compareResult1;
					LONG compareResult2;
					if(compRange->CompareStart(ecReadOnly, selection.range, TF_ANCHOR_START, &compareResult1) == S_OK
						&& compRange->CompareEnd(ecReadOnly, selection.range, TF_ANCHOR_END, &compareResult2) == S_OK) {
						if(compareResult1 == +1 || compareResult2 == -1) {
							// the selection is not entirely in composion
							// end compositon here
							endComposition(pContext);
						}
					}
				}
				selection.range->Release();
			}
		}
	}

	return S_OK;
}


// ITfKeyEventSink
STDMETHODIMP TextService::OnSetFocus(BOOL fForeground) {
	return S_OK;
}

STDMETHODIMP TextService::OnTestKeyDown(ITfContext *pContext, WPARAM wParam, LPARAM lParam, BOOL *pfEaten) {
	if(isKeyboardDisabled(pContext) || !isKeyboardOpened())
		*pfEaten = FALSE;
	else {
		KeyEvent keyEvent(WM_KEYDOWN, wParam, lParam);
		*pfEaten = (BOOL)filterKeyDown(keyEvent);
	}
	return S_OK;
}

STDMETHODIMP TextService::OnKeyDown(ITfContext *pContext, WPARAM wParam, LPARAM lParam, BOOL *pfEaten) {
	// Some applications do not trigger OnTestKeyDown()
	// So we need to test it again here! Windows TSF sucks!
	if(isKeyboardDisabled(pContext) || !isKeyboardOpened())
		*pfEaten = FALSE;
	else {
		KeyEvent keyEvent(WM_KEYDOWN, wParam, lParam);
		*pfEaten = (BOOL)filterKeyDown(keyEvent);
		if(*pfEaten) { // we want to eat the key
			HRESULT sessionResult;
			// ask TSF for an edit session. If editing is approved by TSF,
			// KeyEditSession::DoEditSession will be called, which in turns
			// call back to TextService::doKeyEditSession().
			// So the real key handling is relayed to TextService::doKeyEditSession().
			KeyEditSession* session = new KeyEditSession(this, pContext, keyEvent);

			// We use TF_ES_SYNC here, so the request becomes synchronus and blocking.
			// KeyEditSession::DoEditSession() and TextService::doKeyEditSession() will be
			// called before RequestEditSession() returns.
			pContext->RequestEditSession(clientId_, session, TF_ES_SYNC|TF_ES_READWRITE, &sessionResult);
			*pfEaten = session->result_; // tell TSF if we handled the key
			session->Release();
		}
	}
	return S_OK;
}

STDMETHODIMP TextService::OnTestKeyUp(ITfContext *pContext, WPARAM wParam, LPARAM lParam, BOOL *pfEaten) {
	if(isKeyboardDisabled(pContext) || !isKeyboardOpened())
		*pfEaten = FALSE;
	else {
		KeyEvent keyEvent(WM_KEYDOWN, wParam, lParam);
		*pfEaten = (BOOL)filterKeyUp(keyEvent);
	}
	return S_OK;
}

STDMETHODIMP TextService::OnKeyUp(ITfContext *pContext, WPARAM wParam, LPARAM lParam, BOOL *pfEaten) {
	// Some applications do not trigger OnTestKeyDown()
	// So we need to test it again here! Windows TSF sucks!
	if(isKeyboardDisabled(pContext) || !isKeyboardOpened())
		*pfEaten = FALSE;
	else {
		KeyEvent keyEvent(WM_KEYUP, wParam, lParam);
		*pfEaten = (BOOL)filterKeyUp(keyEvent);
		if(*pfEaten) {
			HRESULT sessionResult;
			KeyEditSession* session = new KeyEditSession(this, pContext, keyEvent);
			pContext->RequestEditSession(clientId_, session, TF_ES_SYNC|TF_ES_READWRITE, &sessionResult);
			*pfEaten = session->result_; // tell TSF if we handled the key
			session->Release();
		}
	}
	return S_OK;
}

STDMETHODIMP TextService::OnPreservedKey(ITfContext *pContext, REFGUID rguid, BOOL *pfEaten) {
	*pfEaten = (BOOL)onPreservedKey(rguid);
	return S_OK;
}


// ITfCompositionSink
STDMETHODIMP TextService::OnCompositionTerminated(TfEditCookie ecWrite, ITfComposition *pComposition) {
	// This is called by TSF when our composition is terminated by others.
	// For example, when the user click on another text editor and the input focus is 
	// grabbed by others, we're ``forced'' to terminate current composition.
	// If we end the composition by calling ITfComposition::EndComposition() ourselves,
	// this event is not triggered.
	onCompositionTerminated(true);

	if(composition_) {
		composition_->Release();
		composition_ = NULL;
	}
	return S_OK;
}

// ITfCompartmentEventSink
STDMETHODIMP TextService::OnChange(REFGUID rguid) {
	// The TSF ITfCompartment is kind of key-value storage
	// It uses GUIDs as keys and stores integer and string values.
	// Global compartment is a storage for cross-process key-value pairs.
	// However, it only handles integers. String values cannot be stored.
	// The thread manager specific compartment, however, handles strings.
	// Every value stored in the storage has an key, which is a GUID.
	// global keyboard states and some other values are in the compartments
	// So we need to monitor for their changes and do some handling.

	// For more detailed introduction, see TSF aware blog:
	// http://blogs.msdn.com/b/tsfaware/archive/2007/05/30/what-is-a-keyboard.aspx

	onCompartmentChanged(rguid);
	return S_OK;
}

// edit session handling
STDMETHODIMP TextService::KeyEditSession::DoEditSession(TfEditCookie ec) {
	EditSession::DoEditSession(ec);
	return textService_->doKeyEditSession(ec, this);
}

// edit session handling
STDMETHODIMP TextService::StartCompositionEditSession::DoEditSession(TfEditCookie ec) {
	EditSession::DoEditSession(ec);
	return textService_->doStartCompositionEditSession(ec, this);
}

// edit session handling
STDMETHODIMP TextService::EndCompositionEditSession::DoEditSession(TfEditCookie ec) {
	EditSession::DoEditSession(ec);
	return textService_->doEndCompositionEditSession(ec, this);
}

// callback from edit session of key events
HRESULT TextService::doKeyEditSession(TfEditCookie cookie, KeyEditSession* session) {
	if(session->keyEvent_.type() == WM_KEYDOWN)
		session->result_ = onKeyDown(session->keyEvent_, session);
	else if(session->keyEvent_.type() == WM_KEYUP)
		session->result_ = onKeyUp(session->keyEvent_, session);
	return S_OK;
}

// callback from edit session for starting composition
HRESULT TextService::doStartCompositionEditSession(TfEditCookie cookie, StartCompositionEditSession* session) {
	ITfContext* context = session->context();
	ITfContextComposition* contextComposition;
	if(context->QueryInterface(IID_ITfContextComposition, (void**)&contextComposition) == S_OK) {
		// get current insertion point in the current context
		ITfRange* range = NULL;
		ITfInsertAtSelection* insertAtSelection;
		if(context->QueryInterface(IID_ITfInsertAtSelection, (void **)&insertAtSelection) == S_OK) {
			// get current selection range & insertion position (query only, did not insert any text)
			insertAtSelection->InsertTextAtSelection(cookie, TF_IAS_QUERYONLY, NULL, 0, &range);
			insertAtSelection->Release();
		}

		if(range) {
			if(contextComposition->StartComposition(cookie, range, (ITfCompositionSink*)this, &composition_) == S_OK) {
				// according to the TSF sample provided by M$, we need to reset current
				// selection here. (maybe the range is altered by StartComposition()?
				// So mysterious. TSF is absolutely overly-engineered!
				TF_SELECTION selection;
				selection.range = range;
				selection.style.ase = TF_AE_NONE;
				selection.style.fInterimChar = FALSE;
				context->SetSelection(cookie, 1, &selection);
				// we did not release composition_ object. we store it for use later
			}
			range->Release();
		}
		contextComposition->Release();
	}
	return S_OK;
}

// callback from edit session for ending composition
HRESULT TextService::doEndCompositionEditSession(TfEditCookie cookie, EndCompositionEditSession* session) {
	if(composition_) {
		// move current insertion point to end of the composition string
		ITfRange* compositionRange;
		if(composition_->GetRange(&compositionRange) == S_OK) {
			// clear display attribute for the composition range
			winrt::com_ptr<ITfProperty> dispAttrProp;
			if(session->context()->GetProperty(GUID_PROP_ATTRIBUTE, dispAttrProp.put()) == S_OK) {
				dispAttrProp->Clear(cookie, compositionRange);
			}

			TF_SELECTION selection;
			ULONG selectionNum;
			if(session->context()->GetSelection(cookie, TF_DEFAULT_SELECTION, 1, &selection, &selectionNum) == S_OK) {
				selection.range->ShiftEndToRange(cookie, compositionRange, TF_ANCHOR_END);
				selection.range->Collapse(cookie, TF_ANCHOR_END);
				session->context()->SetSelection(cookie, 1, &selection);
				selection.range->Release();
			}
			compositionRange->Release();
		}
		// end composition and clean up
		composition_->EndComposition(cookie);
		// do some cleanup in the derived class here
		onCompositionTerminated(false);
		composition_->Release();
		composition_ = NULL;
	}
	return S_OK;
}

ITfContext* TextService::currentContext() {
	ITfContext* context = NULL;
	ITfDocumentMgr  *docMgr;
	if(threadMgr_->GetFocus(&docMgr) == S_OK) {
		docMgr->GetTop(&context);
		docMgr->Release();
	}
	return context;
}

bool TextService::compositionRect(EditSession* session, RECT* rect) {
	bool ret = false;
	if(isComposing()) {
		winrt::com_ptr<ITfContextView> view;
		if(session->context()->GetActiveView(view.put()) == S_OK) {
			BOOL clipped;
			winrt::com_ptr<ITfRange> range;
			if(composition_->GetRange(range.put()) == S_OK) {
				if(view->GetTextExt(session->editCookie(), range.get(), rect, &clipped) == S_OK)
					ret = true;
			}
		}
	}
	return ret;
}

bool TextService::selectionRect(EditSession* session, RECT* rect) {
	bool ret = false;
	winrt::com_ptr<ITfContextView> view;
	if(session->context()->GetActiveView(view.put()) == S_OK) {
		BOOL clipped;
		TF_SELECTION selection;
		ULONG selectionNum;
		if(session->context()->GetSelection(session->editCookie(), TF_DEFAULT_SELECTION, 1, &selection, &selectionNum) == S_OK ) {
			if(view->GetTextExt(session->editCookie(), selection.range, rect, &clipped) == S_OK)
				ret = true;
			selection.range->Release();
		}
	}
	return ret;
}

HWND TextService::compositionWindow(EditSession* session) {
	HWND hwnd = NULL;
	winrt::com_ptr<ITfContextView> view;
	if(session->context()->GetActiveView(view.put()) == S_OK) {
		// get current composition window
		view->GetWnd(&hwnd);
	}
	if (hwnd == NULL)
		hwnd = ::GetFocus();
	return hwnd;
}

} // namespace Ime

